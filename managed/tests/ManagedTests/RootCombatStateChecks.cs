using System.Reflection;
using Advanced_Combat_Tracker;

internal static class RootCombatStateChecks
{
    private static void Check(bool condition, [System.Runtime.CompilerServices.CallerArgumentExpression(nameof(condition))] string? expression = null)
    {
        if (!condition) throw new Exception("Root combat state check failed: " + expression);
    }

    internal static void Run()
    {
        using var root = new FormActMain(new DirectoryInfo(Path.GetTempPath()));
        object? Call(string name, params object[] args) => typeof(FormActMain)
            .GetMethod(name, BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(root, args);
        var epoch = DateTime.UnixEpoch;
        Call("InitializeCombatState", epoch);
        Check(root.ZoneList.Count == 2 && root.ZoneList[0].ZoneName == "Import");
        Check(root.ActiveZone.ZoneName == "Unknown Zone" && root.ActiveZone.PopulateAll);
        Check(!root.InCombat && root.LastHostileTime == DateTime.MinValue);
        var gate = typeof(FormActMain).GetField("CombatGate", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(root);
        var originalGate = typeof(ActGlobals).GetField("actionDataLock", BindingFlags.Static | BindingFlags.NonPublic)!.GetValue(null);
        Check(ReferenceEquals(gate, originalGate));
        Check(typeof(FormActMain).GetProperty("LastHostileTime")!.SetMethod!.IsPrivate);

        var first = (CombatToggleEventArgs)Call("BeginEncounterState", epoch, "Unknown Zone")!;
        Check(first.zoneDataIndex == 1 && first.encounterDataIndex == 1);
        Check(ReferenceEquals(first.encounter, root.ActiveZone.ActiveEncounter));
        Check(!root.InCombat && root.LastHostileTime == DateTime.MinValue);
        Call("MarkEncounterStartedState", epoch);
        Check(root.InCombat && root.LastHostileTime == epoch);
        var zone = root.ActiveZone;
        var encounter = zone.ActiveEncounter;
        Check(Call("BeginEncounterState", epoch.AddSeconds(1), "Different") is null);
        Check(ReferenceEquals(root.ActiveZone, zone) && ReferenceEquals(zone.ActiveEncounter, encounter));
        Check(root.LastHostileTime == epoch.AddSeconds(1));

        Call("ChangeZoneState", "Later", epoch.AddSeconds(10));
        Check(ReferenceEquals(root.ActiveZone, zone));
        root.InCombat = false;
        var later = (CombatToggleEventArgs)Call("BeginEncounterState", epoch.AddSeconds(10), "Later")!;
        Check(root.ZoneList.Count == 3 && root.ActiveZone.ZoneName == "Later");
        Check(later.zoneDataIndex == 2 && later.encounterDataIndex == 1);
        Call("MarkEncounterStartedState", epoch.AddSeconds(10));
        root.InCombat = false;
        // No actions were delivered, so finalization must not invent segment times or a title.
        var ended = (CombatToggleEventArgs)Call("CompleteEncounterState")!;
        Check(ReferenceEquals(ended.encounter, later.encounter) && ended.encounter!.EndTimes.Count == 0);

        Call("ChangeZoneState", "Earlier", epoch.AddSeconds(5));
        var earlier = (CombatToggleEventArgs)Call("BeginEncounterState", epoch.AddSeconds(5), "Earlier")!;
        Check(root.ActiveZone.ZoneName == "Earlier" && ReferenceEquals(root.ActiveZone, root.ZoneList[2]));
        // ACT reports the last list indices even when the event's actual encounter belongs to an older zone.
        Check(earlier.zoneDataIndex == 3 && earlier.encounterDataIndex == 1);
        Check(ReferenceEquals(earlier.encounter, root.ActiveZone.ActiveEncounter));
        Check(!ReferenceEquals(earlier.encounter, root.ZoneList[3].ActiveEncounter));
        Call("MarkEncounterStartedState", epoch.AddSeconds(5));
        root.InCombat = false;
        Call("ChangeZoneState", "Later", epoch.AddSeconds(10));
        var reused = (CombatToggleEventArgs)Call("BeginEncounterState", epoch.AddSeconds(20), "Later")!;
        Check(root.ZoneList.Count == 4 && root.ActiveZone.ZoneName == "Later");
        Check(reused.zoneDataIndex == 3 && reused.encounterDataIndex == 2);
    }
}
