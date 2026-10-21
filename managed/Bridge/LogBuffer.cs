using System.Buffers.Binary;
using System.Text;
using Aftercast.ActCompat;

namespace Aftercast.Bridge;

internal sealed record LogDrain(int Records, byte[] Payload, ulong Dropped, ulong FirstDropped,
    ulong LastDropped, int RequiredPayloadCapacity);

// Own only immutable observations: overflow never waits for consumer progress or mutates ACT's combat graph.
internal sealed class LogBuffer
{
    private const int HeaderLength = 56;
    private const int MaximumBytes = 64 * 1024 * 1024;
    private readonly record struct Entry(OriginalLogRecord Record, ulong Sequence, long UtcTicks, int PathBytes, int LineBytes)
    {
        internal int Length => HeaderLength + PathBytes + LineBytes;
    }
    private static readonly UTF8Encoding Encoding = new(false, true);
    private readonly object gate = new();
    private readonly Queue<Entry> pending = new();
    private ulong lastId, activeId, nextSequence;
    private ulong dropped, firstDropped, lastDropped;
    private int maxRecords, maxBytes, pendingBytes;

    internal ulong Subscribe(int maxRecords, int maxBytes)
    {
        // Validate before inspecting or replacing ownership, including while another subscription is active.
        if (maxRecords is < 1 or > 65536) throw new ArgumentOutOfRangeException(nameof(maxRecords));
        if (maxBytes is < HeaderLength or > MaximumBytes) throw new ArgumentOutOfRangeException(nameof(maxBytes));
        lock (gate)
        {
            if (activeId != 0) throw new InvalidOperationException("A log subscription is already active");
            if (lastId == ulong.MaxValue) throw new InvalidOperationException("Log subscription identifiers are exhausted");
            this.maxRecords = maxRecords;
            this.maxBytes = maxBytes;
            nextSequence = 1;
            activeId = ++lastId;
            return activeId;
        }
    }

    internal void Unsubscribe(ulong id)
    {
        lock (gate) { RequireActive(id); Reset(); }
    }

    private void RequireActive(ulong id)
    {
        if (id == 0 || id != activeId) throw new InvalidOperationException("The log subscription is inactive or stale");
    }

    internal void Capture(OriginalLogRecord record)
    {
        lock (gate)
        {
            if (activeId == 0) return;
            ArgumentNullException.ThrowIfNull(record);
            // Strict byte counting validates text before assigning a sequence or evicting any previous observation.
            var pathBytes = Encoding.GetByteCount(record.Path);
            var lineBytes = Encoding.GetByteCount(record.Line);
            // Local timezone settings may change before drain; freeze the observed instant alongside the original DateTime.
            var utcTicks = record.Time.Kind == DateTimeKind.Unspecified ? -1 : record.Time.ToUniversalTime().Ticks;
            if (nextSequence == 0) throw new InvalidOperationException("Log source sequence identifiers are exhausted");
            var sequence = nextSequence;
            nextSequence = unchecked(sequence + 1);
            var size = (long)HeaderLength + pathBytes + lineBytes;
            while (pending.Count > 0 && (size > maxBytes || pending.Count >= maxRecords || pendingBytes + size > maxBytes))
                DropOldest();
            if (size > maxBytes) { RecordDrop(sequence); return; }
            pending.Enqueue(new(record, sequence, utcTicks, pathBytes, lineBytes));
            pendingBytes += (int)size;
        }
    }

    private void DropOldest()
    {
        var entry = pending.Dequeue();
        pendingBytes -= entry.Length;
        RecordDrop(entry.Sequence);
    }

    private void RecordDrop(ulong sequence)
    {
        // Every eviction is a prefix of the undelivered source. Successful drains reset this interval atomically.
        if (dropped == 0) firstDropped = sequence;
        lastDropped = sequence;
        dropped++;
    }

    internal LogDrain Drain(ulong id, int payloadCapacity)
    {
        if (payloadCapacity is < 0 or > MaximumBytes) throw new ArgumentOutOfRangeException(nameof(payloadCapacity));
        lock (gate)
        {
            RequireActive(id);
            if (pending.TryPeek(out var first) && first.Length > payloadCapacity)
                return new(0, [], 0, 0, 0, first.Length);
            var count = 0;
            var length = 0;
            foreach (var entry in pending)
            {
                if (entry.Length > payloadCapacity - length) break;
                length += entry.Length;
                count++;
            }
            var payload = new byte[length];
            var offset = 0;
            var encoded = 0;
            foreach (var entry in pending)
            {
                if (encoded == count) break;
                Encode(entry, payload.AsSpan(offset, entry.Length));
                offset += entry.Length;
                encoded++;
            }
            // Encoding/allocation failures leave both the records and the loss acknowledgement untouched.
            var result = new LogDrain(count, payload, dropped, firstDropped, lastDropped, 0);
            for (var index = 0; index < count; index++) pendingBytes -= pending.Dequeue().Length;
            dropped = firstDropped = lastDropped = 0;
            return result;
        }
    }

    private static void Encode(Entry entry, Span<byte> destination)
    {
        var record = entry.Record;
        BinaryPrimitives.WriteUInt32LittleEndian(destination, (uint)entry.Length);
        BinaryPrimitives.WriteUInt32LittleEndian(destination[4..], (uint)((int)record.Time.Kind << 1) | (record.IsImport ? 1u : 0u));
        BinaryPrimitives.WriteUInt64LittleEndian(destination[8..], entry.Sequence);
        BinaryPrimitives.WriteInt64LittleEndian(destination[16..], record.Time.Ticks);
        BinaryPrimitives.WriteInt64LittleEndian(destination[24..], entry.UtcTicks);
        BinaryPrimitives.WriteInt32LittleEndian(destination[32..], record.TimeSorter);
        BinaryPrimitives.WriteUInt32LittleEndian(destination[36..], HeaderLength);
        BinaryPrimitives.WriteUInt32LittleEndian(destination[40..], (uint)entry.PathBytes);
        BinaryPrimitives.WriteUInt32LittleEndian(destination[44..], (uint)(HeaderLength + entry.PathBytes));
        BinaryPrimitives.WriteUInt32LittleEndian(destination[48..], (uint)entry.LineBytes);
        BinaryPrimitives.WriteUInt32LittleEndian(destination[52..], 0);
        Encoding.GetBytes(record.Path.AsSpan(), destination.Slice(HeaderLength, entry.PathBytes));
        Encoding.GetBytes(record.Line.AsSpan(), destination.Slice(HeaderLength + entry.PathBytes, entry.LineBytes));
    }

    internal void Clear()
    {
        lock (gate) Reset();
    }

    private void Reset()
    {
        // Host stop retires the current id; a later host session can subscribe without reusing that identity.
        pending.Clear();
        activeId = nextSequence = dropped = firstDropped = lastDropped = 0;
        pendingBytes = 0;
    }
}
