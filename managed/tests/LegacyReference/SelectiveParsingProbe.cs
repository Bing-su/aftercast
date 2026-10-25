using System.Reflection;
using System.Runtime.CompilerServices;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

// Avoid ACT's application constructor; list registration and encounter routing need only this pinned global state.
public static class SelectiveParsingProbe
{
    public static string Run()
    {
        var previousRoot = ActGlobals.oFormActMain;
        var field = typeof(ActGlobals).GetField("selectiveList", BindingFlags.Static | BindingFlags.NonPublic)!;
        var previousList = field.GetValue(null);
        var root = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        GC.SuppressFinalize(root);
        field.SetValue(null, new SortedDictionary<string, bool>());
        ActGlobals.oFormActMain = root;
        try
        {
            root.SelectiveListAdd("source");
            if (!root.SelectiveListGetSelected("SOURCE")) throw new Exception("Selection is not case folded");
            foreach (var ignoreEnemies in new[] { false, true })
                foreach (var selectedSide in new[] { "attacker", "victim", "neither" })
                {
                    var zone = new ZoneData(DateTime.UnixEpoch, "Fixture", false, false, false);
                    var encounter = new EncounterData("Source", "Fixture", ignoreEnemies, zone) { DuplicateDetection = true };
                    var action = new MasterSwing(1, false, "", new Dnum(100), DateTime.UnixEpoch, 1, "Strike",
                        selectedSide == "attacker" ? "Source" : "Other", "Physical", selectedSide == "victim" ? "Source" : "Target");
                    encounter.AddCombatAction(action);
                    var accepted = selectedSide == "attacker" || selectedSide == "victim" && !ignoreEnemies;
                    if (encounter.NumCombatants != (accepted ? 2 : 0) || !ReferenceEquals(action.ParentEncounter, encounter))
                        throw new Exception($"Selective routing differs for {ignoreEnemies}/{selectedSide}");
                    root.SelectiveListClear();
                    encounter.AddCombatAction(action);
                    if (encounter.NumCombatants != (accepted ? 2 : 0)) throw new Exception("Duplicate changed graph");
                    root.SelectiveListAdd("source");
                }
            root.SelectiveListRemove("Source", false);
            if (root.SelectiveListGetSelected("source")) throw new Exception("Uncheck failed");
            root.SelectiveListAdd("Source");
            root.SelectiveListUncheckAll();
            if (root.SelectiveListGetSelected("Source")) throw new Exception("Uncheck all failed");
            root.SelectiveListRemove("Source", true);
            if (((SortedDictionary<string, bool>)field.GetValue(null)!).Count != 0) throw new Exception("Removal failed");
            return "Selective parsing";
        }
        finally { ActGlobals.oFormActMain = previousRoot; field.SetValue(null, previousList); }
    }
}
