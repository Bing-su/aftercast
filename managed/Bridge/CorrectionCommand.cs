using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using Aftercast.ActCompat;

namespace Aftercast.Bridge;

public static partial class EntryPoint
{
    public static int CorrectionCommand(nint buffer, int sizeBytes) => Run(() =>
    {
        const int header = 32, capacity = 4 * 1024 * 1024;
        if (buffer == 0 || sizeBytes != header + capacity) throw new ArgumentException("Invalid correction packet capacity");
        ValidateHeader(buffer, sizeBytes, "Correction ABI size/version mismatch");
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var command = Marshal.ReadInt32(buffer, 16);
        var length = Marshal.ReadInt32(buffer, 20);
        if (command is < 1 or > 4 || length is < 2 or > 128 * 1024 || Marshal.ReadInt64(buffer, 24) != 0)
            throw new ArgumentException("Invalid correction command fields");
        var bytes = new byte[length]; Marshal.Copy(buffer + header, bytes, 0, length);
        var request = JsonSerializer.Deserialize<CorrectionRequest>(new UTF8Encoding(false, true).GetString(bytes), HostJson.Options)
            ?? throw new ArgumentException("Missing correction request");
        var output = plugins.Value.CorrectionOperation(command, request);
        if (output.Length > capacity) throw new InvalidOperationException("Correction reply exceeds bounded capacity");
        Marshal.Copy(output, 0, buffer + header, output.Length);
        Marshal.WriteInt32(buffer, 24, output.Length);
        return 0;
    });
}
internal sealed record CorrectionRequest(ulong Revision = 0, RenameDefinition[]? Renames = null, RedirectDefinition[]? Redirects = null, SpeechRule[]? SpeechRules = null);
