using System.Runtime.InteropServices;
using Bxivact.Bridge;

internal static class LogAbiChecks
{
    internal static void Run(long session)
    {
        var memory = Marshal.AllocHGlobal(128);
        try
        {
            void Check(bool value)
            {
                if (!value) throw new Exception("Log command ABI check failed");
            }
            void Reset(long id = 0, int command = 1)
            {
                Marshal.WriteInt32(memory, 0, 64);
                Marshal.WriteInt32(memory, 4, 1);
                Marshal.WriteInt64(memory, 8, session);
                Marshal.WriteInt64(memory, 16, id);
                Marshal.WriteInt32(memory, 24, command);
                Marshal.WriteInt32(memory, 28, 4);
                Marshal.WriteInt32(memory, 56, 4096);
            }
            void Rejected(Action change, int length, string expected)
            {
                Reset();
                change();
                Check(EntryPoint.LogCommand(memory, length) == -1);
                Check(Program.Diagnostic().Contains(expected, StringComparison.Ordinal));
                Check(Marshal.ReadInt64(memory, 16) == 0);
            }
            Check(EntryPoint.LogCommand(0, 64) == -1);
            Rejected(() => { }, 63, "capacity");
            Rejected(() => { }, 64 + 64 * 1024 * 1024 + 1, "capacity");
            Rejected(() => Marshal.WriteInt32(memory, 0, 65), 64, "size/version");
            Rejected(() => Marshal.WriteInt32(memory, 4, 2), 64, "size/version");
            Rejected(() => Marshal.WriteInt64(memory, 8, session + 1), 64, "does not own");
            Rejected(() => Marshal.WriteInt32(memory, 28, 0), 64, "maxRecords");
            Rejected(() => Marshal.WriteInt32(memory, 56, 55), 64, "maxBytes");
            Rejected(() => Marshal.WriteInt32(memory, 24, 99), 64, "Unknown log subscription");
            Rejected(() => Marshal.WriteInt32(memory, 0, 65), 65, "64-byte");
            Reset();
            Check(EntryPoint.LogCommand(memory, 64) == 0);
            var id = Marshal.ReadInt64(memory, 16);
            Check(id > 0);
            Rejected(() => { }, 64, "already active");
            Reset(id, 2);
            Check(EntryPoint.LogCommand(memory, 64) == 0);
            Check(Marshal.ReadInt32(memory, 28) == 0 && Marshal.ReadInt64(memory, 48) == 0 && Marshal.ReadInt32(memory, 56) == 0);
            Reset(id, 3);
            Marshal.WriteInt32(memory, 0, 65);
            Check(EntryPoint.LogCommand(memory, 65) == -1);
            Reset(id, 2);
            Check(EntryPoint.LogCommand(memory, 64) == 0);
            Reset(id, 3);
            Check(EntryPoint.LogCommand(memory, 64) == 0);
            Reset(id, 2);
            Check(EntryPoint.LogCommand(memory, 64) == -1 && Program.Diagnostic().Contains("stale"));
        }
        finally { Marshal.FreeHGlobal(memory); }
    }
}
