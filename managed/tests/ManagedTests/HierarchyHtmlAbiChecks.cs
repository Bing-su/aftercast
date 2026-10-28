using System.Runtime.InteropServices;
using System.Text;
using Aftercast.Bridge;
internal static class HierarchyHtmlAbiChecks
{
    internal static void Run(long session)
    {
        const int header=32,maximum=16*1024*1024+1024;
        var memory=Marshal.AllocHGlobal(header+maximum);var size=0;
        void Check(bool value){if(!value)throw new Exception("Hierarchy HTML ABI: "+Program.Diagnostic());}
        void Reset(int command,string? json=null)
        {
            size=header+(command==5?4096:maximum);Marshal.Copy(new byte[header],0,memory,header);
            var bytes=Encoding.UTF8.GetBytes(json??(command==5?"{\"html\":{\"encounters\":[1],\"lastLevel\":1}}":"{\"revision\":1,\"htmlLevels\":[{\"level\":1,\"columns\":[\"key\"]}]}"));
            Marshal.WriteInt32(memory,0,size);Marshal.WriteInt32(memory,4,1);Marshal.WriteInt64(memory,8,session);
            Marshal.WriteInt32(memory,16,command);Marshal.WriteInt32(memory,20,bytes.Length);Marshal.Copy(bytes,0,memory+header,bytes.Length);
        }
        try
        {
            foreach(var command in new[]{5,6})
            {
                Reset(command);Check(EntryPoint.TableReportCommand(0,size)==-1);
                foreach(var (offset,value) in new[]{(0,1),(4,2),(20,0),(24,1),(28,1)})
                {Reset(command);Marshal.WriteInt32(memory,offset,value);Check(EntryPoint.TableReportCommand(memory,size)==-1);}
                Reset(command);Marshal.WriteInt64(memory,8,session+1);Check(EntryPoint.TableReportCommand(memory,size)==-1);
                Reset(command);Marshal.WriteByte(memory,header,255);Check(EntryPoint.TableReportCommand(memory,size)==-1);
                foreach(var json in new[]{"null","[]","{}","{\"extra\":1}","{\"revision\":1}","{\"html\":{\"encounters\":[],\"lastLevel\":1}}","{\"html\":{\"encounters\":[1,1],\"lastLevel\":1}}","{\"html\":{\"encounters\":[0],\"lastLevel\":1}}","{\"html\":{\"encounters\":[1],\"lastLevel\":0}}","{\"revision\":1,\"htmlLevels\":[{\"level\":0,\"columns\":[\"key\"]}]}","{\"revision\":1,\"htmlLevels\":[{\"level\":1,\"columns\":[]}]}","{\"revision\":1,\"htmlLevels\":[{\"level\":2,\"columns\":[\"key\"],\"sort\":{\"primary\":\"key\"}}]}"})
                {Reset(command,json);Check(EntryPoint.TableReportCommand(memory,size)==-1);Check(!Program.Diagnostic().Contains("not been configured",StringComparison.Ordinal));}
                Reset(command,"{\"revision\":1,\"revision\":1}");Check(EntryPoint.TableReportCommand(memory,size)==-1);Check(Program.Diagnostic().Contains("Duplicate JSON key",StringComparison.Ordinal));
                Reset(command);Check(EntryPoint.TableReportCommand(memory,size)==-1);Check(Program.Diagnostic().Contains("not been configured",StringComparison.Ordinal));Check(Marshal.ReadInt32(memory,24)==0);
            }
            Reset(5);Marshal.WriteInt32(memory,20,4097);Check(EntryPoint.TableReportCommand(memory,size)==-1);Check(Program.Diagnostic().Contains("request fields",StringComparison.Ordinal));
            Console.WriteLine("Hierarchy HTML ABI capacities, owner, shape/levels/keys, small input bound and strict JSON passed.");
        }
        finally{Marshal.FreeHGlobal(memory);}
    }
}
