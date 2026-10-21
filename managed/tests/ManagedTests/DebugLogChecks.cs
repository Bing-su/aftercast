using System.Diagnostics;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class DebugLogChecks
{
    private sealed class Listener : TraceListener
    {
        internal readonly List<string?> Messages = new();
        internal bool Throw;
        public override void Write(string? message) => WriteLine(message);
        public override void WriteLine(string? message)
        {
            if (Throw) throw new InvalidOperationException("Fixture trace listener failure");
            Messages.Add(message);
        }
    }

    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-debug-");
        using var listener = new Listener();
        Trace.Listeners.Add(listener);
        try
        {
            using var environment = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            root.WriteDebugLog("Fixture debug 日本語");
            if (!listener.Messages.Any(message => message?.Contains("Fixture debug 日本語", StringComparison.Ordinal) == true))
                throw new Exception("ACT debug message did not reach the application trace listener");
            listener.Throw = true;
            root.WriteDebugLog("Listener failure is best effort");
            if (environment.Diagnostics.Item1 != 0)
                throw new Exception("Debug output was incorrectly classified as a host error");
        }
        finally
        {
            Trace.Listeners.Remove(listener);
            directory.Delete(true);
        }
    }
}
