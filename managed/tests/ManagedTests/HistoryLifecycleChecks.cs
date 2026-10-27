using Aftercast.ActCompat;
using Aftercast.Bridge;
using System.Text.Json;

internal static class HistoryLifecycleChecks
{
    private static void Reject(Action action)
    { try { action(); } catch (InvalidOperationException) { return; } throw new Exception("Expected active-operation rejection"); }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-history-lifecycle-");
        var session = new PluginSession(); session.Configure(directory.FullName);
        try
        {
            session.BeginHistory(1,new(new(),null));
            Reject(session.EnsureCanStop);
            Reject(() => session.BeginHistory(1,new(new(),null)));
            Reject(() => session.BeginArchive(3,"",0,false));
            byte[]? output = null;
            if (!SpinWait.SpinUntil(() => { session.Pump(); output = session.AdvanceHistory(); return output is not null; },TimeSpan.FromSeconds(5)))
                throw new Exception("History operation did not complete while owner pumped");
            using var json = JsonDocument.Parse(output!);
            if (json.RootElement.GetProperty("total").GetInt32() != 0) throw new Exception("Cold metadata read unexpectedly restored graph");
            session.BeginArchive(3,"",0,false);
            Reject(() => session.BeginHistory(1,new(new(),null)));
            int? count = null;
            if (!SpinWait.SpinUntil(() => { session.Pump(); count = session.AdvanceArchive(); return count is not null; },TimeSpan.FromSeconds(5)))
                throw new Exception("Archive/history admission stayed busy after completion");
            Reject(() => session.BeginHistory(2,new(null,new(new string('0',64),[new string('a',64)+".act"]))));
            session.EnsureCanStop();
            Console.WriteLine("History Begin/Advance, cold controls, shared admission and stop-before-consumption contracts passed.");
        }
        finally { session.Stop(); directory.Delete(true); }
    }
}
