using System.Reflection;
using System.Runtime.InteropServices;
using Advanced_Combat_Tracker;
using Bxivact.Bridge;

internal static class CombatSnapshotAbiChecks
{
    internal static void Run(long session)
    {
        var memory = Marshal.AllocHGlobal(65536);
        var directory = Directory.CreateTempSubdirectory("bxivact-snapshot-abi-");
        var shared = ((Lazy<PluginSession>)typeof(EntryPoint)
            .GetField("plugins", BindingFlags.NonPublic | BindingFlags.Static)!.GetValue(null)!).Value;
        try
        {
            void Check(bool value)
            {
                if (!value) throw new Exception("Combat snapshot ABI check failed: " + Program.Diagnostic());
            }
            void Reset(int capacity = 64, ulong after = 0)
            {
                Marshal.Copy(new byte[64], 0, memory, 64);
                Marshal.WriteInt32(memory, 0, capacity);
                Marshal.WriteInt32(memory, 4, 1);
                Marshal.WriteInt64(memory, 8, session);
                Marshal.WriteInt64(memory, 16, unchecked((long)after));
            }
            void Rejected(Action change, int length, string expected)
            {
                Reset();
                change();
                Check(EntryPoint.ReadCombatSnapshot(memory, length) == -1);
                Check(Program.Diagnostic().Contains(expected, StringComparison.Ordinal));
            }
            Check(EntryPoint.ReadCombatSnapshot(0, 64) == -1);
            Rejected(() => { }, 63, "capacity");
            Rejected(() => { }, 64 + 64 * 1024 * 1024 + 1, "capacity");
            Rejected(() => Marshal.WriteInt32(memory, 0, 65), 64, "size/version");
            Rejected(() => Marshal.WriteInt32(memory, 4, 2), 64, "size/version");
            Rejected(() => Marshal.WriteInt64(memory, 8, session + 1), 64, "does not own");
            for (var offset = 24; offset < 64; offset += 4)
            {
                var field = offset;
                Rejected(() => Marshal.WriteInt32(memory, field, 1), 64, "initially be zero");
            }
            Rejected(() => Marshal.WriteInt64(memory, 16, 1), 64, "afterRevision");
            Reset();
            Check(EntryPoint.ReadCombatSnapshot(memory, 64) == 0 && Marshal.ReadInt64(memory, 24) == 0);
            shared.Configure(directory.FullName);
            // Initialize the normal host without a fixture plugin dependency; the result contains no actors.
            _ = shared.OnOwner(() => typeof(PluginSession).GetMethod("Host", BindingFlags.NonPublic | BindingFlags.Instance)!.Invoke(shared, null));
            var root = ActGlobals.oFormActMain!;
            root.SetEncounter(DateTime.UnixEpoch, "Source", "Target");
            root.EndCombat(false);
            Reset();
            Check(EntryPoint.ReadCombatSnapshot(memory, 64) == 1);
            var required = Marshal.ReadInt32(memory, 44);
            Check(required > 64 && required < 65536 && Marshal.ReadInt64(memory, 24) == 0);
            Reset(required);
            Check(EntryPoint.ReadCombatSnapshot(memory, required) == 0);
            var revision = unchecked((ulong)Marshal.ReadInt64(memory, 24));
            Check(revision == 1 && Marshal.ReadInt32(memory, 36) == 1 && Marshal.ReadInt32(memory, 44) == 0);
            var payload = new byte[Marshal.ReadInt32(memory, 40)];
            Marshal.Copy(memory + 64, payload, 0, payload.Length);
            Reset(required);
            Check(EntryPoint.ReadCombatSnapshot(memory, required) == 0 && Marshal.ReadInt64(memory, 24) == (long)revision);
            var repeated = new byte[payload.Length];
            Marshal.Copy(memory + 64, repeated, 0, repeated.Length);
            Check(payload.AsSpan().SequenceEqual(repeated));
            Reset(64, revision);
            Check(EntryPoint.ReadCombatSnapshot(memory, 64) == 0 && Marshal.ReadInt64(memory, 24) == 0);
            Reset(64, revision + 1);
            Check(EntryPoint.ReadCombatSnapshot(memory, 64) == -1);
        }
        finally
        {
            shared.Stop();
            Marshal.FreeHGlobal(memory);
            directory.Delete(true);
        }
    }
}
