using System.Runtime.InteropServices;
using System.Text;
using Aftercast.Bridge;

internal static class TableReportAbiChecks
{
    internal static void Run(long session)
    {
        var size = 32 + 56 * 1024 * 1024;
        var memory = Marshal.AllocHGlobal(size);
        void Check(bool value) { if(!value) throw new Exception("Table report ABI: " + Program.Diagnostic()); }
        void Reset(string json="{\"scope\":{\"level\":1,\"encounterId\":1}}",int command=1)
        {
            size = command == 2 ? 32 + 16 * 1024 * 1024 + 20 : 32 + 56 * 1024 * 1024;
            Marshal.Copy(new byte[32],0,memory,32);
            var bytes=Encoding.UTF8.GetBytes(json);
            Marshal.WriteInt32(memory,0,size);Marshal.WriteInt32(memory,4,1);Marshal.WriteInt64(memory,8,session);
            Marshal.WriteInt32(memory,16,command);Marshal.WriteInt32(memory,20,bytes.Length);
            Marshal.Copy(bytes,0,memory+32,bytes.Length);
        }
        try
        {
            Check(EntryPoint.TableReportCommand(0,size)==-1);
            foreach(var (offset,value) in new[]{(0,size-1),(4,2),(16,0),(16,3),(20,0),(20,256*1024+1),(24,1),(28,1)})
            { Reset();Marshal.WriteInt32(memory,offset,value);Check(EntryPoint.TableReportCommand(memory,size)==-1); }
            Reset();Marshal.WriteInt64(memory,8,session+1);Check(EntryPoint.TableReportCommand(memory,size)==-1);
            Reset();Marshal.WriteByte(memory,32,255);Check(EntryPoint.TableReportCommand(memory,size)==-1);
            Reset();Check(EntryPoint.TableReportCommand(memory,size-1)==-1);
            foreach(var json in new[]{"null","[]","{\"extra\":1}","{\"encounterId\":1}","{\"revision\":1}"})
            {Reset(json);Check(EntryPoint.TableReportCommand(memory,size)==-1);}
            foreach(var json in new[]{"{}","{\"encounterId\":1,\"revision\":1}","{\"encounterId\":1,\"revision\":1,\"options\":{}}"})
            {Reset(json,2);Check(EntryPoint.TableReportCommand(memory,size)==-1);}
            foreach(var json in new[]{"{\"revision\":0,\"revision\":0}","{\"options\":{\"sorting\":\"a\",\"sorting\":\"b\"}}"})
            {Reset(json);Check(EntryPoint.TableReportCommand(memory,size)==-1);Check(Program.Diagnostic().Contains("Duplicate JSON key",StringComparison.Ordinal));}
            foreach(var command in new[]{1,2})
            {
                Reset(command == 1 ? "{\"scope\":{\"level\":1,\"encounterId\":1}}" : "{\"revision\":1,\"options\":{\"format\":0,\"name\":\"test\"}}",command);
                Check(EntryPoint.TableReportCommand(memory,size)==-1);
                Check(Program.Diagnostic().Contains("not been configured",StringComparison.Ordinal));
            }
            Check(Marshal.ReadInt32(memory,24)==0);
            Console.WriteLine("Table report ABI capacity/owner/command/strict JSON validation passed.");
        }
        finally {Marshal.FreeHGlobal(memory);}
    }
}
