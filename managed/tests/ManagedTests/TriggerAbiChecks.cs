using System.Runtime.InteropServices;
using System.Text;
using Aftercast.Bridge;

internal static class TriggerAbiChecks
{
    internal static void Run(long session)
    {
        const int size = 32 + 4 * 1024 * 1024;
        var memory = Marshal.AllocHGlobal(size);
        try
        {
            void Reset()
            {
                Marshal.Copy(new byte[size],0,memory,size);
                Marshal.WriteInt32(memory,0,size); Marshal.WriteInt32(memory,4,1);
                Marshal.WriteInt64(memory,8,session); Marshal.WriteInt32(memory,16,1); Marshal.WriteInt32(memory,20,2);
                Marshal.Copy(Encoding.UTF8.GetBytes("{}"),0,memory+32,2);
            }
            void Rejected() { if (EntryPoint.TriggerCommand(memory,size) != -1) throw new Exception("Malformed trigger ABI accepted"); }
            if (EntryPoint.TriggerCommand(0,size) != -1 || EntryPoint.TriggerCommand(memory,32) != -1)
                throw new Exception("Invalid trigger capacity accepted before side effects");
            foreach (var (field,value) in new[] { (4,2),(16,0),(16,8),(20,-1),(20,128*1024+1),(24,1),(28,1) })
            { Reset(); Marshal.WriteInt32(memory,field,value); Rejected(); }
            Reset(); Marshal.WriteInt64(memory,8,session+1); Rejected();
            Reset(); Marshal.WriteByte(memory,32,0xff); Rejected();
            Reset(); Marshal.WriteByte(memory,32,(byte)'['); Marshal.WriteByte(memory,33,(byte)']'); Rejected();
            Console.WriteLine("Trigger ABI capacity-before-mutation, ownership, reserved fields, UTF-8 and JSON checks passed.");
        }
        finally { Marshal.FreeHGlobal(memory); }
    }
}
