using System.Runtime.InteropServices;
using System.Text;
using Aftercast.Bridge;
internal static class HierarchyXmlAbiChecks
{
    internal static void Run(long session)
    {
        const int header=32,maximum=16*1024*1024+32;
        var memory=Marshal.AllocHGlobal(header+maximum);var size=0;
        void Check(bool value){if(!value)throw new Exception("Hierarchy XML ABI: "+Program.Diagnostic());}
        void Reset(int command,string? json=null)
        {
            size=header+(command==3?4096:maximum);Marshal.Copy(new byte[header],0,memory,header);
            var bytes=Encoding.UTF8.GetBytes(json??(command==3?"{\"hierarchy\":{\"encounterId\":1,\"depth\":1}}":"{\"revision\":1}"));
            Marshal.WriteInt32(memory,0,size);Marshal.WriteInt32(memory,4,1);Marshal.WriteInt64(memory,8,session);
            Marshal.WriteInt32(memory,16,command);Marshal.WriteInt32(memory,20,bytes.Length);Marshal.Copy(bytes,0,memory+header,bytes.Length);
        }
        try
        {
            foreach(var command in new[]{3,4})
            {
                Reset(command);Check(EntryPoint.TableReportCommand(0,size)==-1);
                foreach(var (offset,value) in new[]{(0,1),(4,2),(20,0),(24,1),(28,1)})
                {Reset(command);Marshal.WriteInt32(memory,offset,value);Check(EntryPoint.TableReportCommand(memory,size)==-1);}
                Reset(command);Marshal.WriteInt64(memory,8,session+1);Check(EntryPoint.TableReportCommand(memory,size)==-1);
                Reset(command);Marshal.WriteByte(memory,header,255);Check(EntryPoint.TableReportCommand(memory,size)==-1);
                foreach(var json in new[]{"null","[]","{}","{\"extra\":1}","{\"revision\":0}","{\"hierarchy\":{\"encounterId\":0,\"depth\":1}}","{\"hierarchy\":{\"encounterId\":1,\"depth\":0}}"})
                {Reset(command,json);Check(EntryPoint.TableReportCommand(memory,size)==-1);}
                Reset(command,"{\"revision\":1,\"revision\":1}");Check(EntryPoint.TableReportCommand(memory,size)==-1);Check(Program.Diagnostic().Contains("Duplicate JSON key",StringComparison.Ordinal));
                Reset(command);Check(EntryPoint.TableReportCommand(memory,size)==-1);Check(Program.Diagnostic().Contains("not been configured",StringComparison.Ordinal));Check(Marshal.ReadInt32(memory,24)==0);
            }
            Reset(3);Marshal.WriteInt32(memory,20,4097);Check(EntryPoint.TableReportCommand(memory,size)==-1);
            Check(Program.Diagnostic().Contains("request fields",StringComparison.Ordinal));
            Console.WriteLine("Hierarchy XML ABI fixed capacities, owner, shape/depth, small-buffer input bound and strict JSON passed.");
        }
        finally{Marshal.FreeHGlobal(memory);}
    }
}
