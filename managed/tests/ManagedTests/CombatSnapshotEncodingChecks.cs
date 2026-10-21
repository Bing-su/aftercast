using System.Buffers.Binary;
using System.Runtime.CompilerServices;
using System.Text;
using Aftercast.ActCompat;
using Aftercast.Bridge;

internal static class CombatSnapshotEncodingChecks
{
    private static void Check(bool value, [CallerArgumentExpression(nameof(value))] string? expression = null)
    {
        if (!value) throw new Exception("Combat snapshot encoding check failed: " + expression);
    }
    private static void Throws<T>(Action action) where T : Exception
    {
        try { action(); }
        catch (T) { return; }
        throw new Exception("Expected snapshot encoding failure: " + typeof(T).Name);
    }
    private static CombatantSnapshotData Actor(string name = "Actor") => new(name, true,
        long.MinValue, long.MaxValue, -1, 0, -12345,
        BitConverter.Int64BitsToDouble(unchecked((long)0xfff8000000001234)), double.PositiveInfinity, double.NegativeInfinity,
        BitConverter.Int32BitsToSingle(unchecked((int)0xffc01234)),
        int.MinValue, int.MaxValue, -3, 4, -5, 6, -7, 8);
    private static CombatSnapshotData Snapshot(params CombatantSnapshotData[] actors) => new(42, true,
        CombatSnapshotStatus.PartialImport, "영역\0🙂", "Title\0é", "Character𐐀",
        new DateTime(2026, 10, 21, 9, 0, 0, DateTimeKind.Local).AddTicks(123),
        new DateTime(2026, 10, 21, 0, 0, 0, DateTimeKind.Utc).AddTicks(456),
        long.MinValue, long.MaxValue, -1234, actors);

    internal static void Run()
    {
        LayoutAndValues();
        CapacityAndValidation();
        Console.WriteLine("Combat snapshot binary layout, exact values, capacity bounds and strict text validation passed.");
    }

    private static uint U32(ReadOnlySpan<byte> bytes, int offset) => BinaryPrimitives.ReadUInt32LittleEndian(bytes[offset..]);
    private static long I64(ReadOnlySpan<byte> bytes, int offset) => BinaryPrimitives.ReadInt64LittleEndian(bytes[offset..]);
    private static int I32(ReadOnlySpan<byte> bytes, int offset) => BinaryPrimitives.ReadInt32LittleEndian(bytes[offset..]);
    private static string Text(ReadOnlySpan<byte> bytes, uint offset, uint length)
    {
        Check(offset <= bytes.Length && length <= bytes.Length - offset);
        return new UTF8Encoding(false, true).GetString(bytes.Slice((int)offset, (int)length));
    }

    private static void LayoutAndValues()
    {
        var actor = Actor("이름\0🙂");
        var second = actor with { Name = "", IsAlly = false, DPS = -0.0d, ToHit = float.PositiveInfinity };
        var data = Snapshot(actor, second);
        var query = CombatSnapshotEncoding.Encode(data, 0);
        Check(query.Bytes.Length == 0 && query.RequiredPayloadCapacity > 80);
        var tooShort = CombatSnapshotEncoding.Encode(data, query.RequiredPayloadCapacity - 1);
        Check(tooShort.Bytes.Length == 0 && tooShort.RequiredPayloadCapacity == query.RequiredPayloadCapacity);
        var encoded = CombatSnapshotEncoding.Encode(data, query.RequiredPayloadCapacity);
        Check(encoded.RequiredPayloadCapacity == 0 && encoded.Bytes.Length == query.RequiredPayloadCapacity);
        var bytes = encoded.Bytes.AsSpan();
        Check(I64(bytes, 0) == data.Start.Ticks && I64(bytes, 8) == data.End.Ticks);
        Check(I64(bytes, 16) == data.DurationTicks && I64(bytes, 24) == data.Damage && I64(bytes, 32) == data.Healed);
        Check(U32(bytes, 40) == (uint)DateTimeKind.Local && U32(bytes, 44) == (uint)DateTimeKind.Utc);
        Check(U32(bytes, 48) == 80 && U32(bytes, 76) == 0);
        Check(Text(bytes, U32(bytes, 48), U32(bytes, 52)) == data.Zone);
        Check(Text(bytes, U32(bytes, 56), U32(bytes, 60)) == data.Title);
        Check(Text(bytes, U32(bytes, 64), U32(bytes, 68)) == data.Character);
        Check(U32(bytes, 56) == 80 + U32(bytes, 52));
        Check(U32(bytes, 64) == U32(bytes, 56) + U32(bytes, 60));
        Check(U32(bytes, 72) == U32(bytes, 64) + U32(bytes, 68));
        var offset = (int)U32(bytes, 72);
        foreach (var expected in data.Combatants)
        {
            var record = bytes[offset..];
            var length = (int)U32(record, 0);
            Check(length == 120 + Encoding.UTF8.GetByteCount(expected.Name));
            Check(length <= record.Length && U32(record, 4) == (expected.IsAlly ? 1u : 0u));
            Check(I64(record, 8) == expected.Damage && I64(record, 16) == expected.Healed);
            Check(I64(record, 24) == expected.DamageTaken && I64(record, 32) == expected.HealsTaken);
            Check(I64(record, 40) == expected.DurationTicks);
            // Compare floating bits rather than equality so NaN payloads and negative zero are observable.
            Check(I64(record, 48) == BitConverter.DoubleToInt64Bits(expected.DPS));
            Check(I64(record, 56) == BitConverter.DoubleToInt64Bits(expected.EncDPS));
            Check(I64(record, 64) == BitConverter.DoubleToInt64Bits(expected.EncHPS));
            Check(I32(record, 72) == BitConverter.SingleToInt32Bits(expected.ToHit));
            var counters = new[] { expected.Swings, expected.Hits, expected.CritHits, expected.Heals,
                expected.Misses, expected.Blocked, expected.Kills, expected.Deaths };
            for (var index = 0; index < counters.Length; index++) Check(I32(record, 76 + index * 4) == counters[index]);
            Check(U32(record, 108) == 120 && U32(record, 116) == 0);
            Check(Text(record[..length], U32(record, 108), U32(record, 112)) == expected.Name);
            offset += length;
        }
        Check(offset == bytes.Length);
        var unspecified = Snapshot() with
        {
            Zone = "", Title = "", Character = "",
            Start = DateTime.SpecifyKind(DateTime.MinValue, DateTimeKind.Unspecified),
            End = DateTime.SpecifyKind(DateTime.MaxValue, DateTimeKind.Unspecified)
        };
        var empty = CombatSnapshotEncoding.Encode(unspecified, 80).Bytes;
        Check(empty.Length == 80 && I64(empty, 0) == 0 && I64(empty, 8) == DateTime.MaxValue.Ticks);
        Check(U32(empty, 40) == 0 && U32(empty, 44) == 0 && U32(empty, 72) == 80);
    }

    private static void CapacityAndValidation()
    {
        var actor = Actor();
        var valid = Snapshot(actor);
        Throws<ArgumentOutOfRangeException>(() => CombatSnapshotEncoding.Encode(valid, -1));
        Throws<ArgumentOutOfRangeException>(() => CombatSnapshotEncoding.Encode(valid, 64 * 1024 * 1024 + 1));
        Throws<ArgumentNullException>(() => CombatSnapshotEncoding.Encode(null!, 0));
        Throws<ArgumentNullException>(() => CombatSnapshotEncoding.Encode(valid with { Combatants = null! }, 0));
        Throws<ArgumentNullException>(() => CombatSnapshotEncoding.Encode(valid with { Combatants = [null!] }, 0));
        Throws<ArgumentNullException>(() => CombatSnapshotEncoding.Encode(valid with { Zone = null! }, 0));
        Throws<ArgumentNullException>(() => CombatSnapshotEncoding.Encode(valid with { Title = null! }, 0));
        Throws<ArgumentNullException>(() => CombatSnapshotEncoding.Encode(valid with { Character = null! }, 0));
        Throws<ArgumentNullException>(() => CombatSnapshotEncoding.Encode(Snapshot(actor with { Name = null! }), 0));
        foreach (var invalid in new[]
        {
            valid with { Zone = "\ud800" }, valid with { Title = "\udfff" },
            valid with { Character = "\ud800" }, Snapshot(actor with { Name = "\ud800" })
        }) Throws<EncoderFallbackException>(() => CombatSnapshotEncoding.Encode(invalid, 0));
        // A failed query cannot alter the DTO or affect the encoding of a later valid query.
        var before = CombatSnapshotEncoding.Encode(valid, 4096).Bytes;
        Throws<EncoderFallbackException>(() => CombatSnapshotEncoding.Encode(valid with { Zone = "\ud800" }, 4096));
        Check(CombatSnapshotEncoding.Encode(valid, 4096).Bytes.SequenceEqual(before));

        var tooMany = new CombatantSnapshotData[65537];
        Array.Fill(tooMany, actor);
        Throws<ArgumentOutOfRangeException>(() => CombatSnapshotEncoding.Encode(Snapshot(tooMany), 0));
        var maximum = new CombatantSnapshotData[65536];
        Array.Fill(maximum, actor with { Name = "" });
        var maximumData = Snapshot(maximum) with { Zone = "", Title = "", Character = "" };
        Check(CombatSnapshotEncoding.Encode(maximumData, 0).RequiredPayloadCapacity == 80 + 120 * 65536);
        // Warm the query path, then ensure a multi-megabyte required payload does not get allocated during sizing.
        var allocated = GC.GetAllocatedBytesForCurrentThread();
        var query = CombatSnapshotEncoding.Encode(maximumData, 0);
        var queryAllocation = GC.GetAllocatedBytesForCurrentThread() - allocated;
        Check(query.Bytes.Length == 0 && queryAllocation < 65536);
        Array.Fill(maximum, actor with { Name = new string('x', 1024) });
        Throws<InvalidDataException>(() => CombatSnapshotEncoding.Encode(maximumData, 64 * 1024 * 1024));
    }
}
