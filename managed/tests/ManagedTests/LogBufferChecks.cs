using System.Buffers.Binary;
using System.Collections;
using System.Diagnostics;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text;
using Bxivact.ActCompat;
using Bxivact.Bridge;

internal static class LogBufferChecks
{
    private static void Check(bool value, [CallerArgumentExpression(nameof(value))] string? expression = null)
    {
        if (!value) throw new Exception("Log buffer check failed: " + expression);
    }
    private static void Throws<T>(Action action) where T : Exception
    {
        try { action(); }
        catch (T) { return; }
        throw new Exception("Expected log buffer failure: " + typeof(T).Name);
    }
    private static OriginalLogRecord Record(string line = "x") => new(false, "p", line, DateTime.UnixEpoch, 42);
    private static List<ulong> Sequences(LogDrain drain)
    {
        var values = new List<ulong>();
        for (var offset = 0; offset < drain.Payload.Length;)
        {
            var record = drain.Payload.AsSpan(offset);
            var length = BinaryPrimitives.ReadUInt32LittleEndian(record);
            Check(length >= 56 && length <= record.Length);
            values.Add(BinaryPrimitives.ReadUInt64LittleEndian(record[8..]));
            offset += (int)length;
        }
        Check(values.Count == drain.Records);
        return values;
    }

    internal static void Run()
    {
        OwnershipAndValidation();
        OverflowAndAcknowledgement();
        WireEncoding();
        CaptureFreezesLocalInstant();
        HostLifetimeChecks.RunBounded(ConcurrentCaptureAndDrain);
        Console.WriteLine("Bounded original log buffer ownership, loss ranges, encoding and concurrent drain passed.");
    }

    private static void OwnershipAndValidation()
    {
        var buffer = new LogBuffer();
        buffer.Capture(null!); // Disabled capture performs no validation, encoding or sequence assignment.
        foreach (var count in new[] { 0, 65537 }) Throws<ArgumentOutOfRangeException>(() => buffer.Subscribe(count, 56));
        foreach (var size in new[] { 55, 64 * 1024 * 1024 + 1 }) Throws<ArgumentOutOfRangeException>(() => buffer.Subscribe(1, size));
        var first = buffer.Subscribe(1, 56);
        Check(first == 1);
        buffer.Capture(new(false, "", "", DateTime.MinValue, 0));
        Throws<ArgumentOutOfRangeException>(() => buffer.Subscribe(0, 56));
        Throws<InvalidOperationException>(() => buffer.Subscribe(1, 56));
        Throws<InvalidOperationException>(() => buffer.Drain(0, 56));
        Throws<InvalidOperationException>(() => buffer.Unsubscribe(first + 1));
        Throws<ArgumentOutOfRangeException>(() => buffer.Drain(first, -1));
        Throws<ArgumentOutOfRangeException>(() => buffer.Drain(first, 64 * 1024 * 1024 + 1));
        Check(Sequences(buffer.Drain(first, 56)).SequenceEqual(new ulong[] { 1 }));
        buffer.Unsubscribe(first);
        Throws<InvalidOperationException>(() => buffer.Drain(first, 56));
        buffer.Capture(null!);
        var second = buffer.Subscribe(65536, 64 * 1024 * 1024);
        Check(second > first);
        buffer.Capture(Record());
        buffer.Clear();
        Throws<InvalidOperationException>(() => buffer.Unsubscribe(second));
        var third = buffer.Subscribe(1, 58);
        Check(third > second);
        buffer.Capture(Record());
        Check(Sequences(buffer.Drain(third, 58)).SequenceEqual(new ulong[] { 1 }));
        buffer.Clear();
    }

    private static void OverflowAndAcknowledgement()
    {
        var buffer = new LogBuffer();
        var id = buffer.Subscribe(2, 1024);
        buffer.Capture(Record()); buffer.Capture(Record()); buffer.Capture(Record());
        var insufficient = buffer.Drain(id, 57);
        Check(insufficient == new LogDrain(0, insufficient.Payload, 0, 0, 0, 58) && insufficient.Payload.Length == 0);
        var partial = buffer.Drain(id, 58);
        Check(Sequences(partial).SequenceEqual(new ulong[] { 2 }));
        Check(partial.Dropped == 1 && partial.FirstDropped == 1 && partial.LastDropped == 1);
        buffer.Capture(Record()); buffer.Capture(Record());
        var later = buffer.Drain(id, 1024);
        Check(Sequences(later).SequenceEqual(new ulong[] { 4, 5 }));
        Check(later.Dropped == 1 && later.FirstDropped == 3 && later.LastDropped == 3);
        var empty = buffer.Drain(id, 0);
        Check(empty.Records == 0 && empty.Payload.Length == 0 && empty.Dropped == 0 && empty.FirstDropped == 0 && empty.LastDropped == 0);
        buffer.Unsubscribe(id);

        id = buffer.Subscribe(100, 115);
        buffer.Capture(Record()); buffer.Capture(Record());
        var bytes = buffer.Drain(id, 115);
        Check(Sequences(bytes).SequenceEqual(new ulong[] { 2 }) && bytes.Dropped == 1);
        buffer.Capture(Record()); buffer.Capture(Record(new string('x', 200)));
        var oversized = buffer.Drain(id, 0);
        Check(oversized.Records == 0 && oversized.Dropped == 2 && oversized.FirstDropped == 3 && oversized.LastDropped == 4);
        buffer.Capture(Record());
        var recovery = buffer.Drain(id, 115);
        Check(Sequences(recovery).SequenceEqual(new ulong[] { 5 }) && recovery.Dropped == 0);
        // Invalid UTF-16 must fail before evicting a valid pending record or assigning its source sequence.
        buffer.Capture(Record());
        Throws<EncoderFallbackException>(() => buffer.Capture(Record("\ud800")));
        Check(Sequences(buffer.Drain(id, 115)).SequenceEqual(new ulong[] { 6 }));
        buffer.Clear();
    }

    private static void WireEncoding()
    {
        var buffer = new LogBuffer();
        var id = buffer.Subscribe(4, 4096);
        var path = "경로\0🙂";
        var line = "line\0é𐐀";
        var utc = new DateTime(2026, 10, 21, 12, 34, 56, DateTimeKind.Utc).AddTicks(1234);
        var local = DateTime.SpecifyKind(utc, DateTimeKind.Local);
        var unspecified = DateTime.SpecifyKind(utc, DateTimeKind.Unspecified);
        buffer.Capture(new(true, path, line, utc, int.MinValue));
        buffer.Capture(new(false, "", "", local, -1));
        buffer.Capture(new(true, "", "", unspecified, int.MaxValue));
        var drain = buffer.Drain(id, 4096);
        Check(Sequences(drain).SequenceEqual(new ulong[] { 1, 2, 3 }));
        var offset = 0;
        foreach (var expected in new[] { (utc, 3u, int.MinValue), (local, 4u, -1), (unspecified, 1u, int.MaxValue) })
        {
            var bytes = drain.Payload.AsSpan(offset);
            var length = (int)BinaryPrimitives.ReadUInt32LittleEndian(bytes);
            Check(BinaryPrimitives.ReadUInt32LittleEndian(bytes[4..]) == expected.Item2);
            Check(BinaryPrimitives.ReadInt64LittleEndian(bytes[16..]) == expected.Item1.Ticks);
            Check(BinaryPrimitives.ReadInt64LittleEndian(bytes[24..]) == (expected.Item1.Kind == DateTimeKind.Unspecified ? -1 : expected.Item1.ToUniversalTime().Ticks));
            Check(BinaryPrimitives.ReadInt32LittleEndian(bytes[32..]) == expected.Item3);
            var pathOffset = (int)BinaryPrimitives.ReadUInt32LittleEndian(bytes[36..]);
            var pathLength = (int)BinaryPrimitives.ReadUInt32LittleEndian(bytes[40..]);
            var lineOffset = (int)BinaryPrimitives.ReadUInt32LittleEndian(bytes[44..]);
            var lineLength = (int)BinaryPrimitives.ReadUInt32LittleEndian(bytes[48..]);
            Check(pathOffset == 56 && lineOffset == 56 + pathLength && length == lineOffset + lineLength);
            Check(BinaryPrimitives.ReadUInt32LittleEndian(bytes[52..]) == 0);
            Check(Encoding.UTF8.GetString(bytes.Slice(pathOffset, pathLength)) == (offset == 0 ? path : ""));
            Check(Encoding.UTF8.GetString(bytes.Slice(lineOffset, lineLength)) == (offset == 0 ? line : ""));
            offset += length;
        }
        Check(offset == drain.Payload.Length);
        buffer.Clear();
    }

    private static void CaptureFreezesLocalInstant()
    {
        // Windows does not use TZ to select TimeZoneInfo.Local; exercise this process-global Unix contract on Linux only.
        if (!OperatingSystem.IsLinux()) return;
        var previous = Environment.GetEnvironmentVariable("TZ");
        try
        {
            Environment.SetEnvironmentVariable("TZ", "Asia/Seoul");
            TimeZoneInfo.ClearCachedData();
            var local = new DateTime(2026, 1, 1, 9, 0, 0, DateTimeKind.Local);
            var expected = new DateTime(2026, 1, 1, 0, 0, 0, DateTimeKind.Utc);
            Check(local.ToUniversalTime() == expected);
            var buffer = new LogBuffer();
            var id = buffer.Subscribe(1, 56);
            buffer.Capture(new(false, "", "", local, 0));
            Environment.SetEnvironmentVariable("TZ", "UTC");
            TimeZoneInfo.ClearCachedData();
            Check(local.ToUniversalTime() != expected);
            var payload = buffer.Drain(id, 56).Payload;
            Check(BinaryPrimitives.ReadInt64LittleEndian(payload.AsSpan(16)) == local.Ticks);
            Check(BinaryPrimitives.ReadInt64LittleEndian(payload.AsSpan(24)) == expected.Ticks);
            buffer.Clear();
        }
        finally
        {
            Environment.SetEnvironmentVariable("TZ", previous);
            TimeZoneInfo.ClearCachedData();
        }
    }

    private static void CheckBound(LogBuffer buffer, int countLimit, int byteLimit)
    {
        // Inspect actual storage under its monitor without adding a production-only testing API.
        var flags = BindingFlags.Instance | BindingFlags.NonPublic;
        var gate = typeof(LogBuffer).GetField("gate", flags)!.GetValue(buffer)!;
        lock (gate)
        {
            var pending = (ICollection)typeof(LogBuffer).GetField("pending", flags)!.GetValue(buffer)!;
            var bytes = (int)typeof(LogBuffer).GetField("pendingBytes", flags)!.GetValue(buffer)!;
            Check(pending.Count <= countLimit && bytes <= byteLimit && bytes >= 0);
        }
    }

    private static void ConcurrentCaptureAndDrain()
    {
        var buffer = new LogBuffer();
        const int countLimit = 8, byteLimit = 512, sourceCount = 5000;
        var id = buffer.Subscribe(countLimit, byteLimit);
        using var prefixReady = new ManualResetEventSlim();
        using var resume = new ManualResetEventSlim();
        using var done = new ManualResetEventSlim();
        Exception? failure = null;
        var producer = new Thread(() =>
        {
            try
            {
                for (var index = 0; index < sourceCount; index++)
                {
                    buffer.Capture(Record(index.ToString(System.Globalization.CultureInfo.InvariantCulture)));
                    if (index == 999) { prefixReady.Set(); Check(resume.Wait(TimeSpan.FromSeconds(5))); }
                    if (index % 64 == 0) Thread.Yield();
                }
            }
            catch (Exception error) { failure = error; }
            finally { prefixReady.Set(); done.Set(); }
        })
        { IsBackground = true };
        producer.Start();
        Check(prefixReady.Wait(TimeSpan.FromSeconds(5)));
        CheckBound(buffer, countLimit, byteLimit);
        ulong expected = 1, totalDropped = 0;
        resume.Set();
        var started = Stopwatch.GetTimestamp();
        while (true)
        {
            Check(Stopwatch.GetElapsedTime(started) < TimeSpan.FromSeconds(10));
            var finished = done.IsSet;
            var drain = buffer.Drain(id, 128);
            if (drain.Dropped != 0)
            {
                Check(drain.FirstDropped == expected && drain.LastDropped == expected + drain.Dropped - 1);
                expected += drain.Dropped;
                totalDropped += drain.Dropped;
            }
            foreach (var sequence in Sequences(drain)) Check(sequence == expected++);
            CheckBound(buffer, countLimit, byteLimit);
            if (finished && drain.Records == 0) break;
            Thread.Sleep(1); // Deliberately slower consumption exercises overflow; correctness does not depend on a timing threshold.
        }
        Check(producer.Join(TimeSpan.FromSeconds(5)) && failure is null);
        Check(expected == sourceCount + 1 && totalDropped > 0);
        buffer.Clear();
    }
}
