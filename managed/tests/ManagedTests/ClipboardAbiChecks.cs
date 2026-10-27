using System.Runtime.InteropServices;
using System.Text;
using Aftercast.Bridge;

internal static class ClipboardAbiChecks
{
    internal static void Run(long session)
    {
        var memory = Marshal.AllocHGlobal(256);
        try
        {
            var size = 0;
            void Reset(string content = "1|1")
            {
                var bytes = Encoding.UTF8.GetBytes(content); size = 64 + bytes.Length;
                Marshal.Copy(new byte[256], 0, memory, 256);
                Marshal.WriteInt32(memory, 0, size); Marshal.WriteInt32(memory, 4, 1); Marshal.WriteInt64(memory, 8, session);
                Marshal.WriteInt32(memory, 16, 64); Marshal.WriteInt32(memory, 20, bytes.Length); Marshal.WriteInt32(memory, 56, 5);
                Marshal.Copy(bytes, 0, memory + 64, bytes.Length);
            }
            void Reject(Action? change, string expected, int? capacity = null)
            {
                change?.Invoke();
                var before = new byte[64]; Marshal.Copy(memory, before, 0, 64);
                if (EntryPoint.ImportOperation(memory, capacity ?? size) != -1 || !Program.Diagnostic().Contains(expected, StringComparison.Ordinal))
                    throw new Exception("Clipboard ABI rejection: " + expected + " / " + Program.Diagnostic());
                var after = new byte[64]; Marshal.Copy(memory, after, 0, 64);
                if (!before.SequenceEqual(after)) throw new Exception("Rejected clipboard admission mutated its header");
            }
            Reset(); Reject(null, "bounded packet", 63);
            // Oversize validation must precede even reading the fixed header or its claimed payload.
            Reset(); Reject(null, "bounded packet", 64 + 8 * 1024 * 1024 + 1);
            foreach (var field in new[] { 24, 32, 40, 44, 48, 52, 60 })
            { Reset(); Reject(() => Marshal.WriteInt32(memory, field, 1), "clipboard import fields"); }
            foreach (var (field, value) in new[] { (16,63), (16,65), (20,0), (20,-1), (20,int.MaxValue) })
            { Reset(); Reject(() => Marshal.WriteInt32(memory, field, value), "clipboard import fields"); }
            Reset(); Reject(() => Marshal.WriteInt32(memory, 0, size + 1), "ABI mismatch");
            Reset(); Reject(() => Marshal.WriteInt32(memory, 4, 2), "ABI mismatch");
            Reset(); Reject(() => Marshal.WriteInt64(memory, 8, session + 1), "does not own");
            Reset(); Reject(() => Marshal.WriteByte(memory, 64, 255), "translate");
            Reset(); Reject(() => Marshal.Copy(new byte[] { 0xed, 0xa0, 0x80 }, 0, memory + 64, 3), "translate");
            Reset(""); Reject(null, "clipboard import fields");
            Reset("1|1\0한글"); Reject(null, "requires an initialized plugin"); // NUL is text, never a path terminator.
            Reset(); Reject(() => { Marshal.WriteInt32(memory,0,65537); Marshal.WriteInt32(memory,56,4); }, "64..65536",65537);
        }
        finally { Marshal.FreeHGlobal(memory); }
    }
}
