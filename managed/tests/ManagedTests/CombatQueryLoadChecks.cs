using System.Diagnostics;
using System.Security.Cryptography;
using System.Text.Json;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Bridge;
using Aftercast.Testing;

internal static class CombatQueryLoadChecks
{
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-query-load-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            CombatFixture.ConfigureCategories();
            root.SetEncounter(DateTime.UnixEpoch, "Source", "Target");
            for (var index = 0; index < 512; index++)
                root.AddCombatAction(new(1, false, "None", new Dnum(1), DateTime.UnixEpoch.AddSeconds(index),
                    index, "Strike 한글 " + index, "Source", "Physical", "Target"));
            root.EndCombat(true);
            var id = host.ListEncounters(0, 1).Items.Single().Id;
            var total = host.ReadEncounterAttacks(id, 0, 1)!.Total;
            Measure("attack_page", () => CombatQueryEncoding.Encode(host, 4, id, 0, 256).AsMemory());
            Measure("attack_tail", () => CombatQueryEncoding.Encode(host, 4, id, total - 16, 16).AsMemory());
            Measure("encounter_detail", () => CombatQueryEncoding.Encode(host, 3, id, 0, 1).AsMemory());
        }
        finally { directory.Delete(true); }
    }

    private static void Measure(string scenario, Func<ReadOnlyMemory<byte>> encode)
    {
        const int iterations = 2000;
        var expected = encode();
        var length = expected.Length;
        for (var index = 0; index < 200; index++)
            if (!encode().Span.SequenceEqual(expected.Span)) throw new Exception("Combat query payload changed during warmup");
        var before = GC.GetAllocatedBytesForCurrentThread();
        var started = Stopwatch.GetTimestamp();
        for (var index = 0; index < iterations; index++)
            if (encode().Length != length) throw new Exception("Combat query payload changed during measurement");
        var elapsed = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
        var allocated = GC.GetAllocatedBytesForCurrentThread() - before;
        Console.WriteLine(JsonSerializer.Serialize(new
        {
            scenario, iterations, payloadBytes = length,
            payloadSha256 = Convert.ToHexString(SHA256.HashData(expected.Span)),
            allocatedBytes = allocated,
            elapsedMilliseconds = elapsed
        }));
    }
}
