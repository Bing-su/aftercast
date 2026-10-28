using System.Reflection;
using System.Runtime.InteropServices;
using Advanced_Combat_Tracker;
using Aftercast.Bridge;

internal static class CombatQueryAbiChecks
{
    internal static void Run(long session)
    {
        var memory = Marshal.AllocHGlobal(65536);
        var directory = Directory.CreateTempSubdirectory("aftercast-query-abi-");
        var shared = ((Lazy<PluginSession>)typeof(EntryPoint)
            .GetField("plugins", BindingFlags.NonPublic | BindingFlags.Static)!.GetValue(null)!).Value;
        try
        {
            void Check(bool value)
            {
                if (!value) throw new Exception("Combat query ABI check failed: " + Program.Diagnostic());
            }
            void Reset(int capacity = 48, int mode = 2, ulong id = 0)
            {
                Marshal.Copy(new byte[48], 0, memory, 48);
                Marshal.WriteInt32(memory, 0, capacity);
                Marshal.WriteInt32(memory, 4, 1);
                Marshal.WriteInt64(memory, 8, session);
                Marshal.WriteInt32(memory, 16, mode);
                Marshal.WriteInt64(memory, 24, unchecked((long)id));
                Marshal.WriteInt32(memory, 32, 1);
            }
            void Reject(int field, long value, bool wide = false)
            {
                Reset();
                if (wide) Marshal.WriteInt64(memory, field, value);
                else Marshal.WriteInt32(memory, field, checked((int)value));
                Check(EntryPoint.QueryCombat(memory, 48) == -1);
            }
            Check(EntryPoint.QueryCombat(0, 48) == -1);
            Reset();
            Check(EntryPoint.QueryCombat(memory, 47) == -1);
            Check(EntryPoint.QueryCombat(memory, 48 + CombatQueryEncoding.MaximumBytes + 1) == -1);
            Reject(0, 49); Reject(4, 2); Reject(8, session + 1, true);
            Reject(16, 0); Reject(16, 7); Reject(20, -1); Reject(24, 1, true);
            Reject(32, 0); Reject(32, 257); Reject(36, 1); Reject(40, 1); Reject(44, 1);
            Reset(48, 1);
            Marshal.WriteInt32(memory, 20, 1);
            Check(EntryPoint.QueryCombat(memory, 48) == -1);
            Reset(48, 3);
            Check(EntryPoint.QueryCombat(memory, 48) == -1);
            // Capacity retries are read-only, including before any plugin has created an ACT graph.
            Reset();
            Check(EntryPoint.QueryCombat(memory, 48) == 1 && Marshal.ReadInt32(memory, 40) == 56);
            Reset(56);
            Check(EntryPoint.QueryCombat(memory, 56) == 0 && Marshal.ReadInt32(memory, 36) == 8);
            Check(Marshal.ReadInt64(memory, 48) == 0);
            // Reject unknown startup flags before configuring a host or constructing any plugin.
            var directoryBytes = System.Text.Encoding.UTF8.GetBytes(directory.FullName);
            var configureSize = 40 + directoryBytes.Length;
            Reset(configureSize);
            Marshal.WriteInt32(memory, 16, 40); Marshal.WriteInt32(memory, 20, directoryBytes.Length);
            Marshal.WriteInt32(memory, 24, configureSize); Marshal.WriteInt32(memory, 28, 0);
            Marshal.WriteInt64(memory, 32, 4194304);
            Marshal.Copy(directoryBytes, 0, memory + 40, directoryBytes.Length);
            Check(EntryPoint.ConfigureHost(memory, configureSize) == -1);
            shared.Configure(directory.FullName);
            _ = shared.OnOwner(() => typeof(PluginSession).GetMethod("Host", BindingFlags.NonPublic | BindingFlags.Instance)!.Invoke(shared, null));
            var root = ActGlobals.oFormActMain!;
            root.SetEncounter(DateTime.UnixEpoch, "Source", "Target");
            root.EndCombat(false);
            Reset();
            Check(EntryPoint.QueryCombat(memory, 48) == 1);
            var required = Marshal.ReadInt32(memory, 40);
            Reset(required);
            Check(EntryPoint.QueryCombat(memory, required) == 0);
            Check(Marshal.ReadInt32(memory, 48) == 1 && Marshal.ReadInt32(memory, 52) == 1);
            var id = unchecked((ulong)Marshal.ReadInt64(memory, 56));
            Check(id != 0);
            Reset(65536, 3, id);
            Check(EntryPoint.QueryCombat(memory, 65536) == 0 && Marshal.ReadInt32(memory, 48) == 1);
            Check(unchecked((ulong)Marshal.ReadInt64(memory, 52)) == id);
            Reset(65536, 4, id);
            Check(EntryPoint.QueryCombat(memory, 65536) == 0 && Marshal.ReadInt32(memory, 48) == 1);
            Check(Marshal.ReadInt64(memory, 52) == 0);
            Reset(65536, 5, id);
            Check(EntryPoint.QueryCombat(memory, 65536) == 0 && Marshal.ReadInt32(memory, 48) == 1);
            Check(Marshal.ReadInt64(memory, 52) == 0);
            Reset(48, 6, id);
            Check(EntryPoint.QueryCombat(memory, 48) == 1 && Marshal.ReadInt32(memory, 40) == 68);
            Reset(68, 6, id);
            Check(EntryPoint.QueryCombat(memory, 68) == 0 && Marshal.ReadInt32(memory, 48) == 1);
            Check(Marshal.ReadInt64(memory, 52) == 0 && Marshal.ReadInt64(memory, 60) == 0);
            foreach (var mode in new[] { 1, 3, 4, 5, 6 })
            {
                Reset(65536, mode, mode == 1 ? 0 : ulong.MaxValue);
                Check(EntryPoint.QueryCombat(memory, 65536) == 0 && Marshal.ReadInt32(memory, 48) == 0);
            }
        }
        finally
        {
            shared.Stop();
            Marshal.FreeHGlobal(memory);
            directory.Delete(true);
        }
    }
}
