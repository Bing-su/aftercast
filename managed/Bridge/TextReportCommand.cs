using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using Aftercast.ActCompat;

namespace Aftercast.Bridge;

public static partial class EntryPoint
{
    public static int TextReportCommand(nint buffer, int sizeBytes) => Run(() =>
    {
        const int header = 32, capacity = 16 * 1024 * 1024;
        // Reserve the escaped JSON reply before callbacks; never probe capacity by executing twice.
        if (buffer == 0 || sizeBytes != header + capacity) throw new ArgumentException("Invalid report capacity");
        ValidateHeader(buffer, sizeBytes, "Report ABI size/version mismatch");
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var command = Marshal.ReadInt32(buffer, 16);
        var length = Marshal.ReadInt32(buffer, 20);
        if (command is < 1 or > 2 || length is < 2 or > 256 * 1024 || Marshal.ReadInt64(buffer, 24) != 0)
            throw new ArgumentException("Invalid report command fields");
        var bytes = new byte[length]; Marshal.Copy(buffer + header, bytes, 0, length);
        var json = new UTF8Encoding(false, true).GetString(bytes);
        using (var document = JsonDocument.Parse(json, new JsonDocumentOptions { MaxDepth = 16 }))
        {
            if (document.RootElement.ValueKind != JsonValueKind.Object) throw new ArgumentException("Report request must be an object");
            Unique(document.RootElement);
        }
        var request = JsonSerializer.Deserialize<TextReportRequest>(json, HostJson.Options)
            ?? throw new ArgumentException("Missing report request");
        if (command == 1 ? request.EncounterId != 0 || request.Revision != 0 || request.Options is not null :
            request.EncounterId == 0 || request.Revision == 0 || request.Options is null)
            throw new ArgumentException("Report request does not match command");
        var output = plugins.Value.TextReportOperation(request);
        if (output.Length > capacity) throw new InvalidDataException("Report reply exceeds capacity");
        Marshal.Copy(output, 0, buffer + header, output.Length);
        Marshal.WriteInt32(buffer, 24, output.Length);
        return 0;
    });
}
internal sealed record TextReportRequest(ulong EncounterId = 0, ulong Revision = 0, TextReportOptions? Options = null);
