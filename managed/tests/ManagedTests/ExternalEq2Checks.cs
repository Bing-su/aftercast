using System.Security.Cryptography;
using Advanced_Combat_Tracker;
using Aftercast.Bridge;
using Aftercast.TestContracts;

internal static class ExternalEq2Checks
{
    internal static void Run(PluginSession session, IActPluginV1 plugin)
    {
        var root = ActGlobals.oFormActMain!;
        var path = Path.Combine(root.AppDataFolder.FullName, "eq2-fixture.log");
        // Conflicting epoch digits prove that ACT's displayed-date parser is actually used.
        File.WriteAllLines(path, new[]
        {
            "(0000000000)[Thu Jan 01 00:00:00 2026] You have entered Fixture Zone.",
            "(0000000000)[Thu Jan 01 00:00:01 2026] YOU hit Goblin for 100 slashing damage.",
            "(0000000000)[Thu Jan 01 00:00:03 2026] YOU hit Goblin for 300 slashing damage.",
            "(0000000000)[Thu Jan 01 00:00:04 2026] YOUR Heal heals YOU for 50 hit points."
        });
        var mainHash = Hash(plugin.GetType().Assembly.Location);
        using var capture = RecordedActionCapture.Begin(root, new(Hash(path), mainHash, mainHash,
            "934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657"));
        var types = new List<int>();
        root.OnLogLineRead += (_, info) => types.Add(info.detectedType);
        var report = session.ImportLog(path);
        if (report.LinesRead != 4 || report.Encounters != 1 || report.Combatants != 2 || report.Damage != 400 || report.Healed != 50 ||
            !types.SequenceEqual(new[] { 10, 2, 2, 3 }))
            throw new Exception($"EQ2 fixture parsing mismatch: {report}; types={string.Join(',', types)}");
        var encounter = root.ZoneList[0].Items.Single();
        if (encounter.StartTime != new DateTime(2026, 1, 1, 0, 0, 1) || encounter.ZoneName != "Fixture Zone")
            throw new Exception("EQ2 displayed time or zone was lost");
        var transcript = capture.Complete(new[] { new RecordedActionCapture.EncounterExpectation(encounter,
            new[] { new TranscriptActor("YOU", 400, 50, 0, 50), new TranscriptActor("Goblin", 0, 0, 400, 0) },
            new[] { "YOU" }, 400, 50) });
        transcript.Write(Path.Combine(root.AppDataFolder.FullName, "eq2-actions.json"));
        if (!root.OptionsControlSets.ContainsKey(@"Data Correction\EQ2 English Settings") ||
            ActGlobals.oFormEncounterLogs!.LogTypeToColorMapping.Count != 15)
            throw new Exception("EQ2 option/color registrations are missing");
        if (session.ReadCombatSnapshot(0) is not { Damage: 400, Healed: 50 })
            throw new Exception("EQ2 combat result was not published to the owned snapshot");
        root.Disposed += (_, _) =>
        {
            if (root.OptionsControlSets.ContainsKey(@"Data Correction\EQ2 English Settings"))
                throw new Exception("EQ2 did not unregister its settings before host disposal");
        };
        Console.WriteLine("Unmodified EQ2 parser imported four records: 400 damage, 50 healing, two actors and the displayed 2026 timestamp.");
    }
    private static string Hash(string path) => Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(path)));
}
