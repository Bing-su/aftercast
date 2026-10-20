using System.Runtime.InteropServices;
using Bxivact.Bridge;

internal static class DiagnosticAbiChecks
{
    internal static void Run(long session)
    {
        const int capacity = 32 + 1024 * 1024;
        var memory = Marshal.AllocHGlobal(capacity);
        try
        {
            void Check(bool condition)
            {
                if (!condition) throw new Exception("Diagnostic ABI validation failed");
            }
            void Reset()
            {
                Marshal.WriteInt32(memory, 0, capacity);
                Marshal.WriteInt32(memory, 4, 1);
                Marshal.WriteInt64(memory, 8, session);
                Marshal.WriteInt64(memory, 16, -1);
                Marshal.WriteInt64(memory, 24, -1);
            }
            void Rejected(Action change, int size)
            {
                Reset();
                change();
                Check(EntryPoint.ReadDiagnostics(memory, size) == -1);
                Check(Marshal.ReadInt64(memory, 16) == -1 && Marshal.ReadInt64(memory, 24) == -1);
            }
            Check(EntryPoint.ReadDiagnostics(0, capacity) == -1);
            Rejected(() => { }, capacity - 1);
            Rejected(() => Marshal.WriteInt32(memory, 0, 32), capacity);
            Rejected(() => Marshal.WriteInt32(memory, 4, 2), capacity);
            Rejected(() => Marshal.WriteInt64(memory, 8, session + 1), capacity);
            Reset();
            Check(EntryPoint.ReadDiagnostics(memory, capacity) == 0);
            Check(Marshal.ReadInt64(memory, 16) == 0 && Marshal.ReadInt32(memory, 24) == 0 && Marshal.ReadInt32(memory, 28) == 0);
        }
        finally { Marshal.FreeHGlobal(memory); }
    }
}
