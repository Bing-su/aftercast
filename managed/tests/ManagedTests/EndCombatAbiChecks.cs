using Aftercast.Testing;
using System.Drawing;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.Bridge;

internal static class EndCombatAbiChecks
{
    internal static void Run(long session)
    {
        var memory = Marshal.AllocHGlobal(24);
        var directory = Directory.CreateTempSubdirectory("aftercast-end-combat-abi-");
        var shared = ((Lazy<PluginSession>)typeof(EntryPoint)
            .GetField("plugins", BindingFlags.NonPublic | BindingFlags.Static)!.GetValue(null)!).Value;
        void Check(bool condition, [System.Runtime.CompilerServices.CallerArgumentExpression(nameof(condition))] string? expression = null)
        {
            if (!condition) throw new Exception("End combat ABI check failed: " + expression + "; " + Program.Diagnostic());
        }
        void Reset(long owner)
        {
            Marshal.Copy(new byte[24], 0, memory, 24);
            Marshal.WriteInt32(memory, 0, 24);
            Marshal.WriteInt32(memory, 4, 1);
            Marshal.WriteInt32(memory, 8, 5);
            Marshal.WriteInt64(memory, 16, owner);
        }
        try
        {
            Reset(session + 1);
            Check(EntryPoint.Invoke(memory, 24) == 5);
            Reset(session);
            Check(ActGlobals.oFormActMain is null);
            Check(EntryPoint.Invoke(memory, 24) == 0 && ActGlobals.oFormActMain is null);
            shared.Configure(directory.FullName);
            _ = shared.OnOwner(() => typeof(PluginSession).GetMethod("Host", BindingFlags.NonPublic | BindingFlags.Instance)!.Invoke(shared, null));
            var root = ActGlobals.oFormActMain!;
            CombatFixture.ConfigureCategories();
            using var control = shared.OnOwner(() => new Control());
            var ownerThread = Environment.CurrentManagedThreadId;
            var callbackCount = 0;
            root.AfterCombatAction += (_, _) => control.Invoke((Action)(() =>
            {
                Check(Environment.CurrentManagedThreadId == ownerThread && !root.InCombat);
                callbackCount++;
            }));
            // A callback cannot wait for the producer it is currently running in; the rejected command must retain ownership.
            root.BeforeCombatAction += (_, _) =>
            {
                Check(EntryPoint.Invoke(memory, 24) == -1);
                Check(Program.Diagnostic().Contains("Cannot wait for combat actions", StringComparison.Ordinal));
            };
            var time = DateTime.UnixEpoch;
            root.SetEncounter(time, "Source", "Target");
            root.AddCombatAction(new(1, false, "None", new Dnum(100), time, 1, "Strike", "Source", "Physical", "Target"));
            root.AddCombatAction(new(1, false, "None", new Dnum(300), time.AddSeconds(2), 2, "Strike", "Source", "Physical", "Target"));
            Check(root.InCombat);
            Check(EntryPoint.Invoke(memory, 24) == 0 && !root.InCombat && callbackCount == 2);
            var snapshot = shared.ReadCombatSnapshot(0);
            Check(snapshot is { Damage: 400, Healed: 0, IsImport: false } && snapshot.Combatants.Length == 2);
            Check(snapshot!.DurationTicks == TimeSpan.FromSeconds(2).Ticks);
            var revision = snapshot.Revision;
            Check(EntryPoint.Invoke(memory, 24) == 0 && shared.ReadCombatSnapshot(revision) is null);
            Check(shared.ReadCombatSnapshot(0)!.Revision == revision);
            Check(shared.DiagnosticsSnapshot.Item1 == 0);
        }
        finally
        {
            try { shared.Stop(); }
            finally { Marshal.FreeHGlobal(memory); directory.Delete(true); }
        }
    }


}
