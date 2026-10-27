using System.Runtime.InteropServices;

namespace Aftercast.Bridge;

public static partial class EntryPoint
{
    public static int NotificationCommand(nint buffer, int sizeBytes) => Run(() =>
    {
        const int header = 32, capacity = 4 * 1024 * 1024;
        // Validate fixed reply capacity before Show: retrying a short buffer must not repeat callbacks.
        if (buffer == 0 || sizeBytes != header + capacity) throw new ArgumentException("Invalid notification capacity");
        ValidateHeader(buffer, sizeBytes, "Notification ABI size/version mismatch");
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var command = Marshal.ReadInt32(buffer, 16);
        var limit = Marshal.ReadInt32(buffer, 20);
        var id = unchecked((ulong)Marshal.ReadInt64(buffer, 24));
        if (command == 1 ? limit is < 1 or > 16 : command != 2 || limit != 0 || id == 0)
            throw new ArgumentException("Invalid notification command fields");
        var output = plugins.Value.NotificationOperation(command, id, limit);
        if (output.Length > capacity) throw new InvalidOperationException("Notification reply exceeds capacity");
        Marshal.Copy(output, 0, buffer + header, output.Length);
        Marshal.WriteInt32(buffer, 20, output.Length);
        return 0;
    });
}
