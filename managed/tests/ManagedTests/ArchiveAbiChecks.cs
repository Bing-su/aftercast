using System.Runtime.InteropServices;
using Aftercast.Bridge;

internal static class ArchiveAbiChecks
{
    internal static void Run(long session)
    {
        var memory = Marshal.AllocHGlobal(65);
        try
        {
            void Reset(int size = 64)
            {
                Marshal.Copy(new byte[65], 0, memory, 65);
                Marshal.WriteInt32(memory, 0, size); Marshal.WriteInt32(memory, 4, 1);
                Marshal.WriteInt64(memory, 8, session); Marshal.WriteInt32(memory, 16, 3);
                Marshal.WriteInt32(memory, 32, size - 64);
            }
            void Rejected() { if (EntryPoint.ArchiveCommand(memory, Marshal.ReadInt32(memory)) != -1) throw new Exception("Invalid archive ABI was admitted"); }
            if (EntryPoint.ArchiveCommand(0, 64) != -1) throw new Exception("Null archive pointer accepted");
            Reset(); Marshal.WriteInt64(memory, 8, session + 1); Rejected();
            foreach (var (offset, value) in new[] { (4, 2), (16, 0), (16, 5), (20, 1), (24, 1), (32, -1), (36, 1), (40, 1), (48, 1), (56, 1) })
            { Reset(); Marshal.WriteInt32(memory, offset, value); Rejected(); }
            Reset(65); Marshal.WriteInt32(memory, 16, 4); Marshal.WriteByte(memory, 64, 0xff); Rejected();
            Reset(65); Marshal.WriteInt32(memory, 16, 1); Marshal.WriteByte(memory, 64, (byte)'x'); Rejected();
            Reset(65); Marshal.WriteInt32(memory, 16, 4); Rejected();
            Console.WriteLine("Archive ABI session ownership, fields, strict UTF-8 and path bounds passed.");
        }
        finally { Marshal.FreeHGlobal(memory); }
    }
}
