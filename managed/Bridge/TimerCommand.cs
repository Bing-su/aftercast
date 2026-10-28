using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using Aftercast.ActCompat;

namespace Aftercast.Bridge;

public static partial class EntryPoint
{
    public static int TimerCommand(nint buffer, int sizeBytes) => Run(() =>
    {
        const int header = 32, capacity = 4 * 1024 * 1024;
        // Fixed reply space is validated before mutations, so retrying a short buffer cannot repeat a save/delete.
        if (buffer == 0 || sizeBytes != header + capacity) throw new ArgumentException("Invalid timer packet capacity");
        ValidateHeader(buffer, sizeBytes, "Timer ABI size/version mismatch");
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var command = Marshal.ReadInt32(buffer, 16);
        var length = Marshal.ReadInt32(buffer, 20);
        if (command is < 1 or > 5 || length is < 2 or > 128 * 1024 || Marshal.ReadInt64(buffer, 24) != 0)
            throw new ArgumentException("Invalid timer command fields");
        var bytes = new byte[length]; Marshal.Copy(buffer + header, bytes, 0, length);
        var text = new UTF8Encoding(false, true).GetString(bytes);
        var request = JsonSerializer.Deserialize<TimerRequest>(text, HostJson.Options)
            ?? throw new ArgumentException("Missing timer request");
        var output = plugins.Value.TimerOperation(command, request);
        if (output.Length > capacity) throw new InvalidOperationException("Timer reply exceeds bounded capacity");
        Marshal.Copy(output, 0, buffer + header, output.Length);
        Marshal.WriteInt32(buffer, 24, output.Length);
        return 0;
    });
}

internal sealed record TimerRequest(ulong Revision = 0, string? PreviousKey = null, TimerDefinition? Definition = null,
    string? Key = null, int Offset = 0, int Limit = 16, string? Snippet = null);
