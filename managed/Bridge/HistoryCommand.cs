using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using Aftercast.ActCompat;

namespace Aftercast.Bridge;

public static partial class EntryPoint
{
    public static int HistoryOperation(nint buffer,int sizeBytes) => Run(() =>
    {
        const int header = 32, capacity = 4 * 1024 * 1024;
        // Validate before admission; a short reply buffer must never start restoration.
        if (buffer == 0 || sizeBytes != header + capacity) throw new ArgumentException("Invalid history packet capacity");
        ValidateHeader(buffer,sizeBytes,"History ABI size/version mismatch");
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer,8)));
        var command = Marshal.ReadInt32(buffer,16); var length = Marshal.ReadInt32(buffer,20);
        if (Marshal.ReadInt64(buffer,24) != 0) throw new ArgumentException("Invalid history reserved fields");
        if (command == 0)
        {
            if (length != 0) throw new ArgumentException("History poll cannot contain input");
            var output = plugins.Value.AdvanceHistory();
            if (output is null) return 1;
            if (output.Length > capacity) throw new InvalidOperationException("History reply exceeds bounded capacity");
            Marshal.Copy(output,0,buffer+header,output.Length); Marshal.WriteInt32(buffer,24,output.Length);
            return 0;
        }
        if (command is < 1 or > 2 || length is < 2 or > 128 * 1024) throw new ArgumentException("Invalid history command fields");
        var bytes = new byte[length]; Marshal.Copy(buffer+header,bytes,0,length);
        var request = JsonSerializer.Deserialize<HistoryRequest>(new UTF8Encoding(false,true).GetString(bytes),HostJson.Options)
            ?? throw new ArgumentException("Missing history request");
        if (command == 1 ? request.Query is null || request.Selection is not null : request.Selection is null || request.Query is not null)
            throw new ArgumentException("History command payload mismatch");
        plugins.Value.BeginHistory(command,request);
        return 1;
    });
}
internal sealed record HistoryRequest(HistoryQuery? Query = null,HistorySelection? Selection = null);
