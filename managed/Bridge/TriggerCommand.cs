using System.Runtime.InteropServices;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Serialization;
using Aftercast.ActCompat;

namespace Aftercast.Bridge;

public static partial class EntryPoint
{
    public static int TriggerCommand(nint buffer, int sizeBytes) => Run(() =>
    {
        const int header = 32, capacity = 4 * 1024 * 1024;
        // Fixed reply space is validated before mutations, so retrying a short buffer cannot repeat a save/delete.
        if (buffer == 0 || sizeBytes != header + capacity) throw new ArgumentException("Invalid trigger packet capacity");
        ValidateHeader(buffer, sizeBytes, "Trigger ABI size/version mismatch");
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var command = Marshal.ReadInt32(buffer, 16);
        var length = Marshal.ReadInt32(buffer, 20);
        if (command is < 1 or > 11 || length is < 2 or > 128 * 1024 || Marshal.ReadInt64(buffer, 24) != 0)
            throw new ArgumentException("Invalid trigger command fields");
        var bytes = new byte[length]; Marshal.Copy(buffer + header, bytes, 0, length);
        var text = new UTF8Encoding(false, true).GetString(bytes);
        var request = JsonSerializer.Deserialize<TriggerRequest>(text, HostJson.Options)
            ?? throw new ArgumentException("Missing trigger request");
        var output = plugins.Value.TriggerOperation(command, request);
        if (output.Length > capacity) throw new InvalidOperationException("Trigger reply exceeds bounded capacity");
        Marshal.Copy(output, 0, buffer + header, output.Length);
        Marshal.WriteInt32(buffer, 24, output.Length);
        return 0;
    });
}

internal sealed record TriggerRequest(ulong Revision = 0, string? PreviousKey = null, TriggerDefinition? Definition = null,
    string? Key = null, int Offset = 0, int Limit = 16, bool OriginalLines = false, bool ProcessImports = false, ulong After = 0, ulong Snapshot = 0, string[]? Keys = null, string? Snippet = null, TriggerDefinition[]? Definitions = null);

internal static class HostJson
{
    internal static readonly JsonSerializerOptions Options = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase, UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow,
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping, MaxDepth = 16
    };
    internal static byte[] Encode(object value) => JsonSerializer.SerializeToUtf8Bytes(value, Options);
}
