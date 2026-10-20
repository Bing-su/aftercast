using System.Windows.Forms;

namespace Advanced_Combat_Tracker
{
    // Fields, rather than properties, satisfy the original plugin's MemberRefs directly.
    public class ActPluginData
    {
        public FileInfo pluginFile = null!;
        public IActPluginV1 pluginObj = null!;
        public TabPage tpPluginSpace = null!;
        public Label lblPluginStatus = null!;
    }

    public partial class FormActMain(DirectoryInfo appDataFolder) : Form
    {
        private readonly Dictionary<object, ActPluginData> plugins = new(ReferenceEqualityComparer.Instance);
        private readonly object errorGate = new();
        private readonly int ownerThread = Environment.CurrentManagedThreadId;
        private long errorCount, ownerErrorCount;
        private string lastError = "", lastOwnerError = "";
        private readonly Queue<string> recentDiagnostics = new();
        public DirectoryInfo AppDataFolder => appDataFolder;
        public ActPluginData? PluginGetSelfData(IActPluginV1? plugin) =>
            plugin is not null && plugins.TryGetValue(plugin, out var data) ? data : null;
        public void WriteExceptionLog(Exception? error, string context)
        {
            string summary, detail;
            // Plugin-defined exception formatting must not break the host's diagnostic/error path.
            try { summary = error?.Message ?? "Unknown exception"; }
            catch { summary = "Exception message formatting failed"; }
            try { detail = error?.ToString() ?? "Unknown exception"; }
            catch { detail = summary; }
            // Capture caught initializer failures; attribute only logs from the owning thread.
            lock (errorGate)
            {
                errorCount++;
                var message = $"{context}: {summary}";
                lastError = message.Length > 2048 ? message[..2048] : message;
                // Keep bounded stack evidence for worker failures that would otherwise overwrite each other.
                var evidence = $"{context}: {detail}";
                recentDiagnostics.Enqueue(evidence.Length > 8192 ? evidence[..8192] : evidence);
                if (recentDiagnostics.Count > 32) recentDiagnostics.Dequeue();
                if (Environment.CurrentManagedThreadId == ownerThread)
                {
                    ownerErrorCount++;
                    lastOwnerError = lastError;
                }
            }
        }
        public void RestartACT(bool update, string reason) =>
            throw new PlatformNotSupportedException("RestartACT is not supported by an embedded host");
        internal void Register(ActPluginData data) => plugins.Add(data.pluginObj, data);
        internal void Unregister(IActPluginV1 plugin) => plugins.Remove(plugin);
        internal Tuple<long, string> Errors { get { lock (errorGate) return Tuple.Create(errorCount, lastError); } }
        internal Tuple<long, string> OwnerErrors { get { lock (errorGate) return Tuple.Create(ownerErrorCount, lastOwnerError); } }
        internal string[] RecentDiagnostics { get { lock (errorGate) return recentDiagnostics.ToArray(); } }
        internal Tuple<long, string[]> DiagnosticsSnapshot
        {
            get { lock (errorGate) return Tuple.Create(errorCount, recentDiagnostics.ToArray()); }
        }
    }

    public static partial class ActGlobals
    {
        public static FormActMain? oFormActMain;
        public static string? charName;
    }
}

namespace Bxivact.ActCompat
{
    using Advanced_Combat_Tracker;

    // Own host globals separately from the original public ACT contract.
    public class HostEnvironment : IDisposable
    {
        private readonly FormActMain main;
        private bool disposed;
        public HostEnvironment(string directory, Action? pumpCallbacks = null, Func<bool>? isDispatchingCallback = null)
        {
            main = new(new DirectoryInfo(directory));
            main.PumpCallbacks = pumpCallbacks;
            main.IsDispatchingCallback = isDispatchingCallback;
            if (ActGlobals.oFormActMain is not null)
            {
                main.Dispose();
                throw new InvalidOperationException("ACT globals are already owned by another environment");
            }
            ActGlobals.oFormActMain = main;
            ActGlobals.charName = "";
            RegistrationLifetime.Reset();
        }
        public void Register(ActPluginData data) => main.Register(data);
        public void Unregister(IActPluginV1 plugin) => main.Unregister(plugin);
        public Tuple<long, string> Errors => main.OwnerErrors;
        public Tuple<long, string> Diagnostics => main.Errors;
        public string[] RecentDiagnostics => main.RecentDiagnostics;
        public Tuple<long, string[]> DiagnosticsSnapshot => main.DiagnosticsSnapshot;
        public void EnsureCanStop() => main.EnsureCanWaitForLogReader();
        public void StopLogReading() => main.StopLogFollower();
        public void RunWithLogReadingSuspended(Action action) => main.RunWithLogReadingSuspended(action);
        public void Dispose()
        {
            // A previous environment may be disposed again after another host has acquired the globals.
            if (disposed) return;
            main.EnsureCanWaitForLogReader();
            disposed = true;
            try
            {
                // Reader callbacks must finish while their ACT globals still belong to this environment.
                main.StopLogFollower();
            }
            finally
            {
                RegistrationLifetime.Reset();
                ActGlobals.oFormActMain = null;
                ActGlobals.charName = null;
                main.Dispose();
            }
        }
    }
}
