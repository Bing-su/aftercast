using System.Buffers.Binary;
using System.Collections;
using System.Diagnostics;
using System.Reflection;
using System.Text.Json;
using Aftercast.ActCompat;
using Aftercast.Bridge;

// Opt-in synthetic buffer load only: these measurements do not establish network capture or plugin parser throughput.
internal static class LogLoadChecks
{
    private const int SourceRecords = 500_000;
    private const int RecordLimit = 4096;
    private const int ByteLimit = 32 * 1024 * 1024;
    private const int DrainCapacity = 64 * 1024;
    private const int DrainDelayMilliseconds = 2;
    private const int SampleStride = 67; // Coprime with the 8-record size pattern so latency samples represent both line sizes.
    private const string SourcePath = "/synthetic/fixture-2026-10-21.log";
    private static readonly TimeSpan TargetProducerDuration = TimeSpan.FromSeconds(3);
    private static readonly TimeSpan Timeout = TimeSpan.FromSeconds(60);

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException("Synthetic log load failed: " + message);
    }

    internal static void Run()
    {
        var buffer = new LogBuffer();
        var subscription = buffer.Subscribe(RecordLimit, ByteLimit);
        var latencies = new long[(SourceRecords + SampleStride - 1) / SampleStride];
        using var canceled = new ManualResetEventSlim();
        using var producerDone = new ManualResetEventSlim();
        var observer = new BoundObserver(buffer);
        var accounting = new Accounting();
        long captured = 0, captureTicks = 0, producerAllocated = 0;
        double producerSeconds = 0;
        Exception? producerError = null;
        var collections = Enumerable.Range(0, 3).Select(GC.CollectionCount).ToArray();
        var allocatedBefore = GC.GetTotalAllocatedBytes(precise: true);
        var wallStarted = Stopwatch.GetTimestamp();
        var producer = new Thread(() =>
        {
            var started = Stopwatch.GetTimestamp();
            var allocationStart = GC.GetAllocatedBytesForCurrentThread();
            try
            {
                for (var index = 0; index < SourceRecords && !canceled.IsSet; index++)
                {
                    // Allocate a fresh DTO, path and ASCII line for every observation: one in eight lines is 4 KiB, others 512 B.
                    var record = new OriginalLogRecord(false, new string(SourcePath.AsSpan()),
                        new string(index % 8 == 0 ? 'L' : 's', index % 8 == 0 ? 4096 : 512),
                        DateTime.UnixEpoch.AddTicks(index), index);
                    var captureStarted = Stopwatch.GetTimestamp();
                    buffer.Capture(record);
                    var elapsed = Stopwatch.GetTimestamp() - captureStarted;
                    captureTicks += elapsed;
                    if (index % SampleStride == 0) latencies[index / SampleStride] = elapsed;
                    captured++;
                    // Target three seconds of production; exclude deliberate pacing and timer rounding from Capture latency.
                    if ((index + 1) % 1024 == 0 || index + 1 == SourceRecords)
                    {
                        var target = TargetProducerDuration * ((double)(index + 1) / SourceRecords);
                        var remaining = target - Stopwatch.GetElapsedTime(started);
                        if (remaining > TimeSpan.Zero) canceled.Wait(remaining);
                    }
                }
            }
            catch (Exception error) { producerError = error; }
            finally
            {
                producerAllocated = GC.GetAllocatedBytesForCurrentThread() - allocationStart;
                producerSeconds = Stopwatch.GetElapsedTime(started).TotalSeconds;
                producerDone.Set();
            }
        })
        { IsBackground = true, Name = "Aftercast synthetic log producer" };
        var joined = false;
        producer.Start();
        try
        {
            while (true)
            {
                Check(Stopwatch.GetElapsedTime(wallStarted) < Timeout, "producer/drain timed out");
                var finished = producerDone.IsSet;
                observer.Observe();
                var drained = buffer.Drain(subscription, DrainCapacity);
                Check(drained.RequiredPayloadCapacity == 0, "fixed synthetic record did not fit the drain capacity");
                accounting.Accept(drained);
                if (finished && drained.Records == 0) break;
                Thread.Sleep(DrainDelayMilliseconds); // Deliberately slow consumer, fixed across runs; no speed threshold is asserted.
            }
        }
        finally
        {
            canceled.Set();
            joined = producer.Join(TimeSpan.FromSeconds(10));
            buffer.Clear();
        }
        var wallSeconds = Stopwatch.GetElapsedTime(wallStarted).TotalSeconds;
        var allocated = GC.GetTotalAllocatedBytes(precise: true) - allocatedBefore;
        Check(joined, "producer did not join at shutdown");
        if (producerError is not null) throw new InvalidOperationException("Synthetic producer failed", producerError);
        Check(captured == SourceRecords, "not all synthetic source records were captured");
        Check(accounting.NextSequence == SourceRecords + 1UL, "source coverage did not end at the final record");
        Check(accounting.Delivered + accounting.Dropped == SourceRecords, "delivered/lost accounting differs from captured input");
        Array.Sort(latencies);
        double Nanoseconds(long ticks) => (double)ticks * 1_000_000_000 / Stopwatch.Frequency;
        var p50 = latencies[(int)Math.Ceiling(latencies.Length * 0.50) - 1];
        var p99 = latencies[(int)Math.Ceiling(latencies.Length * 0.99) - 1];
        Console.WriteLine(JsonSerializer.Serialize(new
        {
            scenario = "synthetic_managed_original_log_buffer",
            parameters = new
            {
                sourceRecords = SourceRecords,
                smallLineBytes = 512,
                largeLineBytes = 4096,
                largeLineEvery = 8,
                recordLimit = RecordLimit,
                byteLimit = ByteLimit,
                drainCapacity = DrainCapacity,
                drainDelayMilliseconds = DrainDelayMilliseconds,
                targetProducerSeconds = TargetProducerDuration.TotalSeconds,
                latencySampleStride = SampleStride
            },
            result = new
            {
                captured,
                delivered = accounting.Delivered,
                dropped = accounting.Dropped,
                lossIntervals = accounting.LossIntervals,
                payloadBytesDrained = accounting.PayloadBytes,
                wallSeconds,
                producerSeconds,
                captureSeconds = (double)captureTicks / Stopwatch.Frequency,
                sourceRecordsPerWallSecond = captured / wallSeconds,
                captureCallsPerActiveSecond = captureTicks > 0 ? captured * (double)Stopwatch.Frequency / captureTicks : (double?)null,
                captureP50Nanoseconds = Nanoseconds(p50),
                captureP99Nanoseconds = Nanoseconds(p99),
                maxObservedPendingRecords = observer.Records,
                maxObservedPendingBytes = observer.Bytes,
                producerAllocatedBytes = producerAllocated,
                totalAllocatedBytes = allocated,
                gen0Collections = GC.CollectionCount(0) - collections[0],
                gen1Collections = GC.CollectionCount(1) - collections[1],
                gen2Collections = GC.CollectionCount(2) - collections[2],
                producerJoined = joined
            }
        }));
    }

    private sealed class BoundObserver
    {
        private readonly LogBuffer buffer;
        private readonly object gate;
        private readonly FieldInfo queue, bytes;
        internal int Records { get; private set; }
        internal int Bytes { get; private set; }
        internal BoundObserver(LogBuffer buffer)
        {
            this.buffer = buffer;
            const BindingFlags flags = BindingFlags.Instance | BindingFlags.NonPublic;
            gate = typeof(LogBuffer).GetField("gate", flags)!.GetValue(buffer)!;
            queue = typeof(LogBuffer).GetField("pending", flags)!;
            bytes = typeof(LogBuffer).GetField("pendingBytes", flags)!;
        }
        internal void Observe()
        {
            // Read the actual pending storage under its own monitor; avoid a production testing/telemetry API.
            lock (gate)
            {
                var count = ((ICollection)queue.GetValue(buffer)!).Count;
                var length = (int)bytes.GetValue(buffer)!;
                Check(count <= RecordLimit && length is >= 0 and <= ByteLimit, "actual pending storage exceeded its configured bound");
                Records = Math.Max(Records, count);
                Bytes = Math.Max(Bytes, length);
            }
        }
    }

    private sealed class Accounting
    {
        internal ulong NextSequence { get; private set; } = 1;
        internal long Delivered { get; private set; }
        internal long Dropped { get; private set; }
        internal long LossIntervals { get; private set; }
        internal long PayloadBytes { get; private set; }
        internal void Accept(LogDrain drain)
        {
            if (drain.Dropped != 0)
            {
                Check(drain.FirstDropped == NextSequence && drain.LastDropped == NextSequence + drain.Dropped - 1,
                    "loss interval contains a gap, overlap or previously delivered source sequence");
                NextSequence += drain.Dropped;
                Dropped += checked((long)drain.Dropped);
                LossIntervals++;
            }
            else Check(drain.FirstDropped == 0 && drain.LastDropped == 0, "zero loss carried a nonempty interval");
            var count = 0;
            for (var offset = 0; offset < drain.Payload.Length;)
            {
                var record = drain.Payload.AsSpan(offset);
                Check(record.Length >= 56, "truncated encoded header");
                var length = checked((int)BinaryPrimitives.ReadUInt32LittleEndian(record));
                var sequence = BinaryPrimitives.ReadUInt64LittleEndian(record[8..]);
                Check(sequence == NextSequence, "delivered source sequence contains a gap, overlap or reordering");
                Check(BinaryPrimitives.ReadInt32LittleEndian(record[32..]) == checked((int)(sequence - 1)), "sorter no longer identifies its synthetic source");
                Check(length >= 56 && length <= record.Length, "record length exceeds drained payload");
                CheckRecord(record[..length], sequence);
                NextSequence++;
                Delivered++;
                count++;
                offset += length;
            }
            Check(count == drain.Records, "drain record count differs from its encoded payload");
            PayloadBytes += drain.Payload.Length;
        }

        private static void CheckRecord(ReadOnlySpan<byte> record, ulong sequence)
        {
            var sourceIndex = checked((int)(sequence - 1));
            var lineLength = sourceIndex % 8 == 0 ? 4096 : 512;
            var lineOffset = 56 + SourcePath.Length; // This fixed path is ASCII, so its character and UTF-8 lengths coincide.
            Check(record.Length == lineOffset + lineLength, "encoded synthetic path/line size changed");
            Check(BinaryPrimitives.ReadUInt32LittleEndian(record[4..]) == 2, "UTC non-import flags changed");
            Check(BinaryPrimitives.ReadInt64LittleEndian(record[16..]) == DateTime.UnixEpoch.Ticks + sourceIndex &&
                BinaryPrimitives.ReadInt64LittleEndian(record[24..]) == DateTime.UnixEpoch.Ticks + sourceIndex, "timestamp changed");
            Check(BinaryPrimitives.ReadUInt32LittleEndian(record[36..]) == 56 &&
                BinaryPrimitives.ReadUInt32LittleEndian(record[40..]) == SourcePath.Length &&
                BinaryPrimitives.ReadUInt32LittleEndian(record[44..]) == lineOffset &&
                BinaryPrimitives.ReadUInt32LittleEndian(record[48..]) == lineLength &&
                BinaryPrimitives.ReadUInt32LittleEndian(record[52..]) == 0, "encoded text ranges changed");
            Check(record.Slice(56, SourcePath.Length).SequenceEqual("/synthetic/fixture-2026-10-21.log"u8), "source path changed");
            Check(record[lineOffset..].IndexOfAnyExcept(sourceIndex % 8 == 0 ? (byte)'L' : (byte)'s') < 0, "line contents changed");
        }
    }
}
