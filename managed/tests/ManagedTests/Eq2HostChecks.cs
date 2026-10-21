using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class Eq2HostChecks
{
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-eq2-host-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            if (root.TimeStampLen != 39 || !root.LogPathHasCharName || root.LogFileFilter != "eq2log*.txt" ||
                root.GetDateTimeFromLog("(0000000000)[Fri Dec 02 03:04:05 2026] fixture") != new DateTime(2026, 12, 2, 3, 4, 5))
                throw new Exception("EQ2 startup defaults mismatch");
            root.SelectiveListAdd("Alpha");
            if (!root.SelectiveListGetSelected("ALPHA")) throw new Exception("Selective name casing mismatch");
            root.SelectiveListRemove("alpha", false);
            if (root.SelectiveListGetSelected("Alpha")) throw new Exception("Selective uncheck failed");
            root.SelectiveListAdd("Alpha");
            root.SelectiveListUncheckAll();
            if (root.SelectiveListGetSelected("alpha")) throw new Exception("Selective bulk uncheck failed");
            root.SelectiveListAdd("Alpha");
            root.SelectiveListRemove("alpha", true);
            root.SelectiveListAdd("Beta");
            root.SelectiveListClear();
            if (root.SelectiveListGetSelected("Alpha") || root.SelectiveListGetSelected("Beta")) throw new Exception("Selective removal failed");
            if (ActGlobals.oFormEncounterLogs!.LogTypeToColorMapping.Count != 12 || root.OptionsTreeView.Nodes[0].Text != "Data Correction")
                throw new Exception("Option/log registrations were not initialized");
        }
        finally { directory.Delete(true); }
        if (ActGlobals.oFormEncounterLogs is not null) throw new Exception("Encounter log global survived disposal");
    }
}
