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
        public DirectoryInfo AppDataFolder => appDataFolder;
        public ActPluginData? PluginGetSelfData(IActPluginV1? plugin) =>
            plugin is not null && plugins.TryGetValue(plugin, out var data) ? data : null;
        public void WriteExceptionLog(Exception? error, string context)
        {
            // Capture caught initializer failures; attribute only logs from the owning thread.
            lock (errorGate)
            {
                errorCount++;
                var message = $"{context}: {error?.Message ?? "Unknown exception"}";
                lastError = message.Length > 2048 ? message[..2048] : message;
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
    }

    public static class ActGlobals
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
        public HostEnvironment(string directory)
        {
            main = new(new DirectoryInfo(directory));
            if (ActGlobals.oFormActMain is not null)
            {
                main.Dispose();
                throw new InvalidOperationException("ACT globals are already owned by another environment");
            }
            ActGlobals.oFormActMain = main;
            ActGlobals.charName = "";
        }
        public void Register(ActPluginData data) => main.Register(data);
        public void Unregister(IActPluginV1 plugin) => main.Unregister(plugin);
        public Tuple<long, string> Errors => main.OwnerErrors;
        public Tuple<long, string> Diagnostics => main.Errors;
        public void Dispose()
        {
            // A previous environment may be disposed again after another host has acquired the globals.
            if (disposed) return;
            disposed = true;
            // Clear globals before disposal so a later session cannot observe a disposed root.
            ActGlobals.oFormActMain = null;
            ActGlobals.charName = null;
            main.Dispose();
        }
    }
}
