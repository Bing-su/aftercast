using System.Diagnostics;
using System.Security.Cryptography;
using System.Text.Json;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class ArchiveLoadChecks
{
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-archive-load-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            var payload = new byte[2 * 1024 * 1024];
            for (var index = 0; index < payload.Length; index++) payload[index] = (byte)index;
            var compressed = root.MemCompressGZip(payload);
            for (var index = 0; index < 8; index++)
                if (!root.MemDecompressGzip(compressed).AsSpan().SequenceEqual(payload))
                    throw new Exception("Archive warmup changed payload bytes");
            const int iterations = 32;
            var before = GC.GetAllocatedBytesForCurrentThread();
            var started = Stopwatch.GetTimestamp();
            for (var index = 0; index < iterations; index++)
                if (!root.MemDecompressGzip(compressed).AsSpan().SequenceEqual(payload))
                    throw new Exception("Archive decompression changed payload bytes");
            var elapsed = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
            var allocated = GC.GetAllocatedBytesForCurrentThread() - before;
            Console.WriteLine(JsonSerializer.Serialize(new
            {
                scenario = "gzip_decompression", iterations, payloadBytes = payload.Length,
                payloadSha256 = Convert.ToHexString(SHA256.HashData(payload)),
                allocatedBytes = allocated, elapsedMilliseconds = elapsed
            }));
        }
        finally { directory.Delete(true); }
    }
}
