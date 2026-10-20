using System.Buffers.Binary;
using System.Text;
using Bxivact.ActCompat;

namespace Bxivact.Bridge;

internal sealed record CombatSnapshotPayload(byte[] Bytes, int RequiredPayloadCapacity);

// Encode an owned graph snapshot only; capacity queries never allocate the potentially large native payload.
internal static class CombatSnapshotEncoding
{
    private const int HeaderLength = 80;
    private const int ActorHeaderLength = 120;
    private const int MaximumBytes = 64 * 1024 * 1024;
    private static readonly UTF8Encoding Encoding = new(false, true);

    internal static CombatSnapshotPayload Encode(CombatSnapshotData data, int payloadCapacity)
    {
        if (payloadCapacity is < 0 or > MaximumBytes) throw new ArgumentOutOfRangeException(nameof(payloadCapacity));
        ArgumentNullException.ThrowIfNull(data);
        ArgumentNullException.ThrowIfNull(data.Combatants);
        if (data.Combatants.Length > 65536) throw new ArgumentOutOfRangeException(nameof(data), "A snapshot cannot contain more than 65536 combatants");
        var zoneBytes = TextLength(data.Zone);
        var titleBytes = TextLength(data.Title);
        var characterBytes = TextLength(data.Character);
        var titleOffset = AddLength(HeaderLength, zoneBytes);
        var characterOffset = AddLength(titleOffset, titleBytes);
        var actorsOffset = AddLength(characterOffset, characterBytes);
        var length = actorsOffset;
        foreach (var actor in data.Combatants)
        {
            ArgumentNullException.ThrowIfNull(actor);
            length = AddLength(length, AddLength(ActorHeaderLength, TextLength(actor.Name)));
        }
        if (payloadCapacity < length) return new([], length);

        // All text and checked lengths have been validated before allocating any payload storage.
        var payload = new byte[length];
        var bytes = payload.AsSpan();
        BinaryPrimitives.WriteInt64LittleEndian(bytes, data.Start.Ticks);
        BinaryPrimitives.WriteInt64LittleEndian(bytes[8..], data.End.Ticks);
        BinaryPrimitives.WriteInt64LittleEndian(bytes[16..], data.DurationTicks);
        BinaryPrimitives.WriteInt64LittleEndian(bytes[24..], data.Damage);
        BinaryPrimitives.WriteInt64LittleEndian(bytes[32..], data.Healed);
        // ACT graph times have no original timezone offset; preserve wall-clock ticks and Kind without conversion.
        BinaryPrimitives.WriteUInt32LittleEndian(bytes[40..], (uint)data.Start.Kind);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes[44..], (uint)data.End.Kind);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes[48..], HeaderLength);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes[52..], (uint)zoneBytes);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes[56..], (uint)titleOffset);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes[60..], (uint)titleBytes);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes[64..], (uint)characterOffset);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes[68..], (uint)characterBytes);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes[72..], (uint)actorsOffset);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes[76..], 0);
        Encoding.GetBytes(data.Zone.AsSpan(), bytes.Slice(HeaderLength, zoneBytes));
        Encoding.GetBytes(data.Title.AsSpan(), bytes.Slice(titleOffset, titleBytes));
        Encoding.GetBytes(data.Character.AsSpan(), bytes.Slice(characterOffset, characterBytes));
        var offset = actorsOffset;
        foreach (var actor in data.Combatants)
        {
            var nameBytes = TextLength(actor.Name);
            var actorLength = AddLength(ActorHeaderLength, nameBytes);
            EncodeActor(actor, bytes.Slice(offset, actorLength), nameBytes);
            offset += actorLength;
        }
        return new(payload, 0);
    }

    private static int TextLength(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        return Encoding.GetByteCount(value);
    }

    private static int AddLength(int first, int second)
    {
        var result = checked((long)first + second);
        if (result > MaximumBytes) throw new InvalidDataException("The combat snapshot payload exceeds 64 MiB");
        return checked((int)result);
    }

    private static void EncodeActor(CombatantSnapshotData actor, Span<byte> bytes, int nameBytes)
    {
        BinaryPrimitives.WriteUInt32LittleEndian(bytes, (uint)bytes.Length);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes[4..], actor.IsAlly ? 1u : 0u);
        BinaryPrimitives.WriteInt64LittleEndian(bytes[8..], actor.Damage);
        BinaryPrimitives.WriteInt64LittleEndian(bytes[16..], actor.Healed);
        BinaryPrimitives.WriteInt64LittleEndian(bytes[24..], actor.DamageTaken);
        BinaryPrimitives.WriteInt64LittleEndian(bytes[32..], actor.HealsTaken);
        BinaryPrimitives.WriteInt64LittleEndian(bytes[40..], actor.DurationTicks);
        // Writing the floating representations directly retains NaN payloads, infinities and negative zero.
        BinaryPrimitives.WriteDoubleLittleEndian(bytes[48..], actor.DPS);
        BinaryPrimitives.WriteDoubleLittleEndian(bytes[56..], actor.EncDPS);
        BinaryPrimitives.WriteDoubleLittleEndian(bytes[64..], actor.EncHPS);
        BinaryPrimitives.WriteSingleLittleEndian(bytes[72..], actor.ToHit);
        BinaryPrimitives.WriteInt32LittleEndian(bytes[76..], actor.Swings);
        BinaryPrimitives.WriteInt32LittleEndian(bytes[80..], actor.Hits);
        BinaryPrimitives.WriteInt32LittleEndian(bytes[84..], actor.CritHits);
        BinaryPrimitives.WriteInt32LittleEndian(bytes[88..], actor.Heals);
        BinaryPrimitives.WriteInt32LittleEndian(bytes[92..], actor.Misses);
        BinaryPrimitives.WriteInt32LittleEndian(bytes[96..], actor.Blocked);
        BinaryPrimitives.WriteInt32LittleEndian(bytes[100..], actor.Kills);
        BinaryPrimitives.WriteInt32LittleEndian(bytes[104..], actor.Deaths);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes[108..], ActorHeaderLength);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes[112..], (uint)nameBytes);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes[116..], 0);
        Encoding.GetBytes(actor.Name.AsSpan(), bytes.Slice(ActorHeaderLength, nameBytes));
    }
}
