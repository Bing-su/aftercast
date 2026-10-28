using System.Runtime.InteropServices;
using System.Text;

namespace Aftercast.Bridge;

public static partial class EntryPoint
{
    public static int ArchiveCommand(nint buffer, int sizeBytes) => Run(() => ArchiveCore(buffer, sizeBytes, false));

    public static int ArchiveOperation(nint buffer, int sizeBytes) => Run(() => ArchiveCore(buffer, sizeBytes, true));

    // Async polling uses the same validated packet layout as the synchronous compatibility entry point.
    private static int ArchiveCore(nint buffer, int sizeBytes, bool asynchronous)
    {
        if (buffer == 0 || sizeBytes is < 64 or > 65536) throw new ArgumentException("Archive command requires 64..65536 bytes");
        ValidateHeader(buffer, sizeBytes, "Archive ABI size/version mismatch");
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var command = Marshal.ReadInt32(buffer, 16);
        if (asynchronous && command == 0)
        {
            if (sizeBytes != 64) throw new ArgumentException("Archive poll requires 64 bytes");
            for (var offset = 20; offset < 64; offset += 4)
                if (Marshal.ReadInt32(buffer, offset) != 0) throw new ArgumentException("Invalid archive poll fields");
            var completed = plugins.Value.AdvanceArchive();
            if (completed is null) return 1;
            Marshal.WriteInt64(buffer, 48, completed.Value);
            return 0;
        }
        var flags = Marshal.ReadInt32(buffer, 20);
        var id = unchecked((ulong)Marshal.ReadInt64(buffer, 24));
        var length = Marshal.ReadInt32(buffer, 32);
        if (command is < 1 or > 4 || flags is < 0 or > 3 || command == 1 && flags > 1 || command is 3 or 4 && flags != 0 ||
            (command == 2 ? id == 0 : id != 0) || length != sizeBytes - 64 ||
            Marshal.ReadInt32(buffer, 36) != 0 || Marshal.ReadInt64(buffer, 40) != 0 || Marshal.ReadInt64(buffer, 48) != 0 || Marshal.ReadInt64(buffer, 56) != 0)
            throw new ArgumentException("Invalid archive command fields");
        var bytes = new byte[length]; Marshal.Copy(buffer + 64, bytes, 0, length);
        var text = new UTF8Encoding(false, true).GetString(bytes);
        if (text.Contains('\0') || command is 1 or 2 && !Path.IsPathFullyQualified(text) ||
            command == 3 && text.Length != 0 || command == 4 && text.Length > 4096)
            throw new ArgumentException("Invalid archive path or history filter");
        Marshal.WriteInt64(buffer, 48, 0);
        if (asynchronous)
        {
            plugins.Value.BeginArchive(command, text, id, command == 2 && (flags & 1) != 0, (flags & 2) == 0, command == 1 && (flags & 1) != 0);
            return 1;
        }
        var count = plugins.Value.ArchiveOperation(command, text, id, command == 2 && (flags & 1) != 0, (flags & 2) == 0, command == 1 && (flags & 1) != 0);
        Marshal.WriteInt64(buffer, 48, count);
        return 0;
    }
}
