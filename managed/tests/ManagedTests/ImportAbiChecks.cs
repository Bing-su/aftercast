using System.Runtime.InteropServices;
using System.Text;
using Aftercast.Bridge;

internal static class ImportAbiChecks
{
    internal static void Run(long session)
    {
        var memory = Marshal.AllocHGlobal(65537);
        try
        {
            var path = Encoding.UTF8.GetBytes(Path.GetFullPath("missing-import.log"));
            var size = 64 + path.Length;
            void Check(bool value)
            {
                if (!value) throw new Exception("Import ABI validation failed");
            }
            void Reset()
            {
                Marshal.WriteInt32(memory, 0, size);
                Marshal.WriteInt32(memory, 4, 1);
                Marshal.WriteInt64(memory, 8, session);
                Marshal.WriteInt32(memory, 16, 64);
                Marshal.WriteInt32(memory, 20, path.Length);
                for (var offset = 24; offset < 64; offset += 8) Marshal.WriteInt64(memory, offset, -1);
                Marshal.Copy(path, 0, memory + 64, path.Length);
            }
            void Rejected(Action change, int length, string expected)
            {
                Reset();
                change();
                Check(EntryPoint.ImportLog(memory, length) == -1);
                Check(Program.Diagnostic().Contains(expected, StringComparison.Ordinal));
                // Rejected requests cannot overwrite a result or start import callbacks.
                for (var offset = 24; offset < 64; offset += 8) Check(Marshal.ReadInt64(memory, offset) == -1);
            }
            Check(EntryPoint.ImportLog(0, 64) == -1);
            Rejected(() => { }, 63, "64..65536");
            Rejected(() => { }, 65537, "64..65536");
            Rejected(() => Marshal.WriteInt32(memory, 0, 64), size, "size/version");
            Rejected(() => Marshal.WriteInt32(memory, 4, 2), size, "size/version");
            Rejected(() => Marshal.WriteInt64(memory, 8, session + 1), size, "does not own");
            Rejected(() => Marshal.WriteInt32(memory, 16, 63), size, "outside");
            Rejected(() => Marshal.WriteInt32(memory, 16, int.MaxValue), size, "outside");
            Rejected(() => Marshal.WriteInt32(memory, 20, -1), size, "outside");
            Rejected(() => Marshal.WriteInt32(memory, 20, int.MaxValue), size, "outside");
            Rejected(() => Marshal.WriteInt32(memory, 20, 0), size, "outside");
            Rejected(() => Marshal.WriteByte(memory, 64, 0), size, "contain no NUL");
            Rejected(() => Marshal.WriteByte(memory, 64, 255), size, "translate");
            Rejected(() => Marshal.Copy(Encoding.UTF8.GetBytes("rel"), 0, memory + 64, 3), size, "must be absolute");
            Rejected(() => { }, size, "requires an initialized plugin");
            void StepRejected(Action change, string expected)
            {
                Reset();
                for (var offset = 24; offset < 64; offset += 8) Marshal.WriteInt64(memory, offset, 0);
                Marshal.WriteInt32(memory, 56, 4);
                change();
                Check(EntryPoint.ImportOperation(memory, size) == -1);
                Check(Program.Diagnostic().Contains(expected, StringComparison.Ordinal));
            }
            StepRejected(() => Marshal.WriteInt64(memory, 24, -1), "ticks");
            StepRejected(() => Marshal.WriteInt64(memory, 32, DateTime.MaxValue.Ticks + 1), "ticks");
            StepRejected(() => Marshal.WriteInt64(memory, 24, 1), "start must not follow end");
            StepRejected(() => Marshal.WriteInt64(memory, 40, 1), "reserved");
            StepRejected(() => Marshal.WriteInt32(memory, 60, 1), "reserved");
            StepRejected(() => { Marshal.WriteInt32(memory, 56, 1); Marshal.WriteInt64(memory, 32, 1); }, "must not specify");
            StepRejected(() => { }, "requires an initialized plugin");
        }
        finally { Marshal.FreeHGlobal(memory); }
    }
}
