using System.Runtime.InteropServices;
using System.Text;
using System.Runtime.CompilerServices;
using Aftercast.Bridge;

internal static class TableAbiChecks
{
    internal static void Run(long session)
    {
        const int size = 32 + 16 * 1024 * 1024;
        var memory = Marshal.AllocHGlobal(size);
        void Check(bool value, [CallerArgumentExpression(nameof(value))] string? expression = null) { if (!value) throw new Exception("Table ABI contract failed: " + expression + " / " + Program.Diagnostic()); }
        try
        {
            void Reset(string json = "{\"query\":null}", int command = 2)
            {
                Marshal.Copy(new byte[32], 0, memory, 32);
                var bytes = Encoding.UTF8.GetBytes(json);
                Marshal.WriteInt32(memory, 0, command == 1 ? 32 + 4 * 1024 * 1024 : size);
                Marshal.WriteInt32(memory, 4, 1); Marshal.WriteInt64(memory, 8, session);
                Marshal.WriteInt32(memory, 16, command); Marshal.WriteInt32(memory, 20, bytes.Length);
                Marshal.Copy(bytes, 0, memory + 32, bytes.Length);
            }
            void Rejected(Action change)
            {
                Reset(); change(); Check(EntryPoint.TableCommand(memory, size) == -1);
            }
            Check(EntryPoint.TableCommand(0, size) == -1);
            Check(EntryPoint.TableCommand(memory, 31) == -1);
            foreach (var (offset, value) in new[] { (4, 2), (16, 0), (16, 3), (20, -1), (20, 256 * 1024 + 1), (24, 1), (28, 1) })
                Rejected(() => Marshal.WriteInt32(memory, offset, value));
            Rejected(() => Marshal.WriteInt64(memory, 8, session + 1));
            Rejected(() => Marshal.WriteByte(memory, 32, 255));
            Reset(); Check(EntryPoint.TableCommand(memory, size - 1) == -1);
            foreach (var json in new[] { "null", "[]", "{\"unknown\":1}", "{\"level\":0}", "{\"level\":0,\"query\":{}}" })
            { Reset(json); Check(EntryPoint.TableCommand(memory, size) == -1); }
            Reset("{\"level\":99}", 1); Check(EntryPoint.TableCommand(memory, 32 + 4 * 1024 * 1024) == -1);
            Reset("{\"level\":1}", 1);
            Check(EntryPoint.TableCommand(memory, size) == -1); // Wrong capacity is rejected before catalog access.
            // This ABI fixture intentionally has no configured Host; the valid request must reach that guard, not fabricate a catalog.
            Check(EntryPoint.TableCommand(memory, 32 + 4 * 1024 * 1024) == -1);
            Check(Program.Diagnostic().Contains("not been configured", StringComparison.Ordinal));
            Check(Marshal.ReadInt32(memory, 24) == 0);
            Console.WriteLine("Table ABI fixed capacities, ownership, reserved fields, strict input and unconfigured-host guard passed.");
        }
        finally { Marshal.FreeHGlobal(memory); }
    }
}
