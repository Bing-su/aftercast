using System.Runtime.InteropServices;
using Aftercast.Bridge;

internal static class NotificationAbiChecks
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
                Marshal.WriteInt64(memory,8,session); Marshal.WriteInt32(memory,16,1); Marshal.WriteInt32(memory,20,16);
            }
            void Rejected() { if (EntryPoint.NotificationCommand(memory,size) != -1) throw new Exception("Malformed notification ABI accepted"); }
            if (EntryPoint.NotificationCommand(0,size) != -1 || EntryPoint.NotificationCommand(memory,32) != -1)
                throw new Exception("Notification capacity accepted before callback");
            foreach (var (field,value) in new[] { (0,32),(4,2),(16,0),(16,6),(20,0),(20,17) })
            { Reset(); Marshal.WriteInt32(memory,field,value); Rejected(); }
            Reset(); Marshal.WriteInt64(memory,8,session+1); Rejected();
            Reset(); Marshal.WriteInt32(memory,16,2); Marshal.WriteInt32(memory,20,0); Rejected();
            Reset(); Marshal.WriteInt32(memory,16,2); Marshal.WriteInt64(memory,24,1); Rejected();
            foreach (var command in new[] { 3, 4, 5 })
            {
                Reset(); Marshal.WriteInt32(memory,16,command); Rejected(); // nonzero limit
            }
            Reset(); Marshal.WriteInt32(memory,16,5); Marshal.WriteInt32(memory,20,0);
            Marshal.WriteInt64(memory,24,1); Rejected();
            Console.WriteLine("Notification ABI capacity-before-callback, session and field validation passed.");
        }
        finally { Marshal.FreeHGlobal(memory); }
    }
}
