using System.Drawing;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

// Verify real object ownership and mutable collections without claiming combat aggregation.
public static class CombatStateProbe
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Original-reference combat state probe failed");
    }

    public static string Run()
    {
        var charName = ActGlobals.charName;
        var outgoing = CombatantData.OutgoingDamageTypeDataObjects;
        var incoming = CombatantData.IncomingDamageTypeDataObjects;
        try
        {
            ActGlobals.charName = "Fixture Player";
            foreach (var populateAll in new[] { false, true })
            foreach (var selective in new[] { false, true })
            foreach (var ignoreEnemies in new[] { false, true })
            {
                var zone = new ZoneData(DateTime.UnixEpoch, "Fixture Zone", populateAll, selective, ignoreEnemies);
                Check(zone.PopulateAll == populateAll && zone.StartTime == DateTime.UnixEpoch);
                Check(zone.ZoneName == "Fixture Zone" && zone.ActiveEncounter is null);
                Check(zone.Items.Count == (populateAll ? 1 : 0) && zone.Tags.Count == 0);
                if (populateAll)
                {
                    var merged = zone.Items[0];
                    Check(ReferenceEquals(merged.Parent, zone) && merged.CharName == "Fixture Player");
                    Check(merged.Title == "All" && merged.ZoneName == "Fixture Zone");
                    zone.ZoneName = "Renamed Zone";
                    merged.Title = "Ignored for merged encounter";
                    Check(merged.ZoneName == "Renamed Zone" && merged.Title == "All");
                }
            }

            var parent = new ZoneData(DateTime.UnixEpoch, "Parent Zone", false, false, false);
            var encounter = new EncounterData("Named Player", "Own Zone", false, parent);
            Check(encounter.CharName == "Named Player" && ReferenceEquals(encounter.Parent, parent));
            Check(encounter.ZoneName == "Own Zone" && encounter.Title == "Encounter");
            Check(!encounter.Active && !encounter.DuplicateDetection && encounter.HistoryRecord is null);
            Check(encounter.Items.Count == 0 && encounter.LogLines.Count == 0 && encounter.Tags.Count == 0);
            encounter.Title = "Renamed Encounter";
            encounter.CharName = "Changed Player";
            Check(encounter.Title == "Renamed Encounter" && encounter.CharName == "Changed Player");
            var starts = new List<DateTime> { DateTime.MaxValue, DateTime.UnixEpoch, DateTime.MaxValue };
            var ends = new List<DateTime> { DateTime.MinValue, DateTime.UnixEpoch, DateTime.MinValue };
            encounter.StartTimes = starts;
            encounter.EndTimes = ends;
            Check(ReferenceEquals(starts, encounter.StartTimes) && starts.SequenceEqual(new[] { DateTime.UnixEpoch }));
            Check(ReferenceEquals(ends, encounter.EndTimes) && ends.SequenceEqual(new[] { DateTime.UnixEpoch }));

            CombatantData.OutgoingDamageTypeDataObjects = new()
            {
                ["Outgoing first"] = new("First", 0, Color.Red),
                ["Outgoing last"] = new("Last", 0, Color.Blue)
            };
            CombatantData.IncomingDamageTypeDataObjects = new()
            {
                ["Incoming"] = new("Incoming", 0, Color.Green)
            };
            var combatant = new CombatantData("Combatant", encounter);
            Check(combatant.Name == "Combatant" && ReferenceEquals(combatant.Parent, encounter));
            Check(encounter.Items.Count == 0 && combatant.Items.Count == 3);
            Check(combatant.Allies.Count == 0 && combatant.Tags.Count == 0);
            Check(ReferenceEquals(combatant.AllOut, combatant.Items["Outgoing last"].Items));
            Check(ReferenceEquals(combatant.AllInc, combatant.Items["Incoming"].Items));
            foreach (var item in combatant.Items)
                Check(ReferenceEquals(item.Value.Parent, combatant) && item.Value.Outgoing == item.Key.StartsWith("Outgoing", StringComparison.Ordinal));
            encounter.Items.Add(combatant.Name, combatant);
            Check(encounter.NumCombatants == 1);

            var line = new LogLineEntry(DateTime.UnixEpoch, "raw line", 7, 42);
            Check(line.Time == DateTime.UnixEpoch && line.LogLine == "raw line" && line.Type == 7);
            Check(line.GlobalTimeSorter == 42 && !line.SearchSelected);
            line.SearchSelected = true;
            line.Time = DateTime.UnixEpoch.AddSeconds(1);
            encounter.LogLines.Add(line);
            Check(ReferenceEquals(encounter.LogLines[0], line) && line.SearchSelected);
            var history = new HistoryRecord(2, DateTime.UnixEpoch, line.Time, "Label", "Player", "Folder");
            encounter.HistoryRecord = history;
            Check(ReferenceEquals(encounter.HistoryRecord, history) && history.Type == 2);
            Check(history.CharName == "Player" && history.Label == "Label" && history.FolderHint == "Folder");
            Check(history.Duration == TimeSpan.FromSeconds(1));
            return "Combat state";
        }
        finally
        {
            ActGlobals.charName = charName;
            CombatantData.OutgoingDamageTypeDataObjects = outgoing;
            CombatantData.IncomingDamageTypeDataObjects = incoming;
        }
    }
}
