using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class DiagnosticChecks
{
    private sealed class BrokenFormatting : Exception
    {
        public override string Message => throw new InvalidOperationException("Broken Message");
        public override string ToString() => throw new InvalidOperationException("Broken ToString");
    }

    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-diagnostics-");
        try
        {
            using var environment = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            root.WriteExceptionLog(new BrokenFormatting(), "broken");
            if (environment.Diagnostics.Item1 != 1 || !environment.Diagnostics.Item2.Contains("formatting failed"))
                throw new Exception("Plugin-defined exception formatting broke diagnostics");
            for (var index = 0; index < 40; index++)
                root.WriteExceptionLog(new Exception(new string('x', 9000)), index.ToString());
            var recent = environment.RecentDiagnostics;
            if (environment.Diagnostics.Item1 != 41 || recent.Length != 32 || recent.Any(item => item.Length > 8192) || !recent[0].StartsWith("8:"))
                throw new Exception("Diagnostic retention exceeded bounds or lost total count");
        }
        finally { directory.Delete(true); }
    }
}
