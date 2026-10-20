using System.Drawing;
using System.Runtime.CompilerServices;
using System.Text;
using Advanced_Combat_Tracker;
using Bxivact.ActCompat;

internal static class CombatSnapshotChecks
{
    private static readonly DateTime Epoch = DateTime.UnixEpoch;
    private static void Check(bool condition, [CallerArgumentExpression(nameof(condition))] string? expression = null)
    {
        if (!condition) throw new Exception("Combat snapshot check failed: " + expression);
    }
    private static void Throws<T>(Action action) where T : Exception
    {
        try { action(); }
        catch (T) { return; }
        throw new Exception("Expected snapshot failure: " + typeof(T).Name);
    }

    private sealed class Fixture : IDisposable
    {
        internal readonly DirectoryInfo Directory = System.IO.Directory.CreateTempSubdirectory("bxivact-combat-snapshot-");
        internal readonly HostEnvironment Host;
        internal readonly FormActMain Root;
        internal string Path => System.IO.Path.Combine(Directory.FullName, "import.log");
        internal Fixture()
        {
            Host = new(Directory.FullName);
            Root = ActGlobals.oFormActMain!;
            ActGlobals.charName = "Source";
            CombatantData.DamageTypeDataOutgoingDamage = "Damage";
            CombatantData.DamageTypeDataOutgoingHealing = "Healing";
            CombatantData.DamageTypeDataIncomingDamage = "Taken";
            CombatantData.DamageTypeDataIncomingHealing = "Received";
            CombatantData.OutgoingDamageTypeDataObjects = new()
            {
                ["Damage"] = new("Damage", -1, Color.Red),
                ["Healing"] = new("Healing", 1, Color.Green),
                ["All Out"] = new("All", 0, Color.Transparent)
            };
            CombatantData.IncomingDamageTypeDataObjects = new()
            {
                ["Taken"] = new("Taken", -1, Color.Red),
                ["Received"] = new("Received", 1, Color.Green),
                ["All In"] = new("All", 0, Color.Transparent)
            };
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = new() { [1] = new() { "Damage" }, [3] = new() { "Healing" } };
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming = new() { [1] = new() { "Taken" }, [3] = new() { "Received" } };
            Root.LastKnownTime = Epoch;
            Root.ChangeZone("Fixture Zone");
            Root.GetDateTimeFromLog = line => line == "first" ? Epoch : Epoch.AddSeconds(2);
        }
        internal void Add(int swing, long amount, int second, int sorter, bool critical = false)
        {
            Root.SetEncounter(Epoch.AddSeconds(second), "Source", "Target");
            Root.AddCombatAction(new(swing, critical, "None", new Dnum(amount), Epoch.AddSeconds(second), sorter,
                swing == 3 ? "Heal" : "Strike", "Source", swing == 3 ? "Healing" : "Physical", swing == 3 ? "Source" : "Target"));
        }
        public void Dispose()
        {
            try { Host.Dispose(); }
            finally { Directory.Delete(true); }
        }
    }

    internal static void Run()
    {
        HostLifetimeChecks.RunBounded(LiveResults);
        HostLifetimeChecks.RunBounded(CaptureFailures);
        HostLifetimeChecks.RunBounded(ImportResults);
        Console.WriteLine("Owned combat snapshots, version queries and partial import status passed.");
    }

    private static void LiveResults()
    {
        using var fixture = new Fixture();
        Check(fixture.Host.ReadCombatSnapshot(0) is null);
        Throws<ArgumentOutOfRangeException>(() => fixture.Host.ReadCombatSnapshot(1));
        var sawUnpublished = false;
        CombatToggleEventDelegate unpublished = (_, _) => sawUnpublished = fixture.Host.ReadCombatSnapshot(0) is null;
        fixture.Root.OnCombatEnd += unpublished;
        fixture.Add(1, 100, 0, 1);
        fixture.Add(3, 20, 1, 2);
        fixture.Add(1, 300, 2, 3, true);
        fixture.Root.EndCombat(true);
        fixture.Root.OnCombatEnd -= unpublished;
        var first = fixture.Host.ReadCombatSnapshot(0)!;
        Check(sawUnpublished && first.Revision == 1 && !first.IsImport && first.Status == CombatSnapshotStatus.Ended);
        Check(first.Zone == "Fixture Zone" && first.Title == "Target" && first.Character == "Source");
        Check(first.Start == Epoch && first.End == Epoch.AddSeconds(2) && first.DurationTicks == TimeSpan.FromSeconds(2).Ticks);
        Check(first.Damage == 400 && first.Healed == 20 && first.Combatants.Length == 2);
        var source = first.Combatants.Single(actor => actor.Name == "Source");
        var target = first.Combatants.Single(actor => actor.Name == "Target");
        Check(source.IsAlly && !target.IsAlly);
        Check(source.Damage == 400 && source.Healed == 20 && source.DamageTaken == 0 && source.HealsTaken == 20);
        Check(source.DurationTicks == first.DurationTicks && source.DPS == 200 && source.EncDPS == 200 && source.EncHPS == 10);
        Check(source.ToHit == 100 && source.Swings == 2 && source.Hits == 2 && source.CritHits == 1 && source.Heals == 1);
        Check(source.Misses == 0 && source.Blocked == 0 && source.Kills == 0 && source.Deaths == 0);
        Check(target.DamageTaken == 400 && target.Damage == 0 && target.DurationTicks == 0);
        Check(double.IsNaN(target.DPS) && float.IsNaN(target.ToHit));
        Check(fixture.Host.ReadCombatSnapshot(first.Revision) is null);
        Throws<ArgumentOutOfRangeException>(() => fixture.Host.ReadCombatSnapshot(first.Revision + 1));
        var completed = fixture.Root.ActiveZone.ActiveEncounter!;
        var sourceActor = completed.Items["SOURCE"];
        var damageCategory = sourceActor.Items["Damage"];
        try
        {
            completed.Title = "Changed after completion";
            sourceActor.Items.Remove("Damage");
            var unchanged = fixture.Host.ReadCombatSnapshot(0)!;
            Check(unchanged.Title == "Target" && unchanged.Damage == 400);
            Check(unchanged.Combatants.Single(actor => actor.Name == "Source").Damage == 400);
            Check(fixture.Host.ReadCombatSnapshot(first.Revision) is null);
        }
        finally { sourceActor.Items.Add("Damage", damageCategory); }
        Check(fixture.Host.ReadCombatSnapshot(0)!.Revision == first.Revision);
        // Returning the same revision twice still gives each caller a separate actor array.
        var independent = fixture.Host.ReadCombatSnapshot(0)!;
        independent.Combatants[0] = source with { Damage = -123 };
        Check(fixture.Host.ReadCombatSnapshot(0)!.Combatants.All(actor => actor.Damage != -123));
        fixture.Add(1, 7, 3, 4);
        fixture.Root.EndCombat(false);
        fixture.Add(1, 9, 4, 5);
        fixture.Root.EndCombat(false);
        var latest = fixture.Host.ReadCombatSnapshot(first.Revision)!;
        Check(latest.Revision == first.Revision + 2 && latest.Damage == 9);
        Check(first.Damage == 400 && source.Damage == 400 && first.Combatants.Length == 2);
        // The returned array is caller-owned; altering it cannot replace internal encounter objects.
        first.Combatants[0] = source with { Damage = -123 };
        Check(fixture.Host.ReadCombatSnapshot(0)!.Combatants.All(actor => actor.Damage != -123));
    }

    private static void CaptureFailures()
    {
        using var fixture = new Fixture();
        SortedList<string, CombatantData>? savedActors = null;
        EncounterData? malformed = null;
        // End notifications are legitimate ACT mutation points; extraction must not make their result fatal to EndCombat.
        CombatToggleEventDelegate oversize = (_, info) =>
        {
            malformed = info.encounter!;
            savedActors = malformed.Items;
            var tooMany = new SortedList<string, CombatantData>(65537);
            for (var index = 0; index < 65537; index++)
                tooMany.Add(index.ToString("D5", System.Globalization.CultureInfo.InvariantCulture), savedActors["SOURCE"]);
            malformed.Items = tooMany;
        };
        fixture.Root.OnCombatEnd += oversize;
        fixture.Add(1, 100, 0, 1);
        fixture.Root.EndCombat(true);
        fixture.Root.OnCombatEnd -= oversize;
        Throws<InvalidOperationException>(() => fixture.Host.ReadCombatSnapshot(0));
        malformed!.Items = savedActors!;
        // Restoring graph state cannot silently repair an already published failure under the same revision.
        Throws<InvalidOperationException>(() => fixture.Host.ReadCombatSnapshot(0));

        CombatantData? brokenActor = null;
        DamageTypeData? savedCategory = null;
        CombatToggleEventDelegate missingCategory = (_, info) =>
        {
            brokenActor = info.encounter!.Items["SOURCE"];
            savedCategory = brokenActor.Items["Received"];
            brokenActor.Items.Remove("Received");
        };
        fixture.Root.OnCombatEnd += missingCategory;
        fixture.Add(1, 200, 1, 2);
        fixture.Root.EndCombat(true);
        fixture.Root.OnCombatEnd -= missingCategory;
        Throws<KeyNotFoundException>(() => fixture.Host.ReadCombatSnapshot(1));
        brokenActor!.Items.Add("Received", savedCategory!);
        Throws<KeyNotFoundException>(() => fixture.Host.ReadCombatSnapshot(1));
        fixture.Add(1, 300, 2, 3);
        fixture.Root.EndCombat(true);
        var recovered = fixture.Host.ReadCombatSnapshot(2)!;
        Check(recovered.Revision == 3 && recovered.Damage == 300);
        Check(fixture.Host.ReadCombatSnapshot(3) is null);
        Check(fixture.Host.Errors.Item1 == 0);
    }

    private static void ImportResults()
    {
        using var fixture = new Fixture();
        fixture.Root.OnLogLineRead += (_, info) =>
        {
            fixture.Root.SetEncounter(info.detectedTime, "Source", "Target");
            fixture.Root.AddCombatAction(new(1, false, "None", new Dnum(100), info.detectedTime,
                fixture.Root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
        };
        var pending = new List<CombatSnapshotStatus>();
        fixture.Root.LogFileChanged += (import, _) =>
        {
            if (!import && fixture.Host.ReadCombatSnapshot(0) is { } snapshot) pending.Add(snapshot.Status);
        };
        File.WriteAllText(fixture.Path, "first\nsecond\n", new UTF8Encoding(false));
        fixture.Host.ImportLog(fixture.Path);
        var success = fixture.Host.ReadCombatSnapshot(0)!;
        Check(success.IsImport && success.Status == CombatSnapshotStatus.Ended && success.Damage == 200);
        Check(pending.Last() == CombatSnapshotStatus.Importing);
        File.WriteAllBytes(fixture.Path, Encoding.UTF8.GetBytes("first\n").Concat(new byte[] { 0xff }).ToArray());
        Throws<InvalidDataException>(() => fixture.Host.ImportLog(fixture.Path));
        var partial = fixture.Host.ReadCombatSnapshot(success.Revision)!;
        Check(partial.IsImport && partial.Status == CombatSnapshotStatus.PartialImport && partial.Damage == 100);
        Check(partial.Revision == success.Revision + 2);
        File.WriteAllBytes(fixture.Path, Array.Empty<byte>());
        fixture.Host.ImportLog(fixture.Path);
        Check(fixture.Host.ReadCombatSnapshot(partial.Revision) is null);
        Throws<FileNotFoundException>(() => fixture.Host.ImportLog(System.IO.Path.Combine(fixture.Directory.FullName, "absent.log")));
        Check(fixture.Host.ReadCombatSnapshot(partial.Revision) is null);
        File.WriteAllText(fixture.Path, "first\n", new UTF8Encoding(false));
        fixture.Root.LogFileChanged += (import, _) =>
        {
            if (import) return;
            fixture.Add(1, 17, 10, 9900);
            fixture.Root.EndCombat(true);
        };
        fixture.Host.ImportLog(fixture.Path);
        var live = fixture.Host.ReadCombatSnapshot(partial.Revision)!;
        Check(!live.IsImport && live.Status == CombatSnapshotStatus.Ended && live.Damage == 17);
        Check(live.Revision == partial.Revision + 2);
    }
}
