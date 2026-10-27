using System.Runtime.InteropServices;
using System.Text;

namespace Aftercast.Bridge;

public static partial class EntryPoint
{
    // Step commands retain no native pointers, e.g. a pending disk read outlives only managed state.
    public static int ImportOperation(nint buffer, int sizeBytes) => Run(() =>
    {
        if (buffer == 0 || sizeBytes < 64 || sizeBytes > 65536)
            throw new ArgumentException("Import operation requires 64..65536 bytes");
        ValidateHeader(buffer, sizeBytes, "Import operation ABI mismatch");
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var command = Marshal.ReadInt32(buffer, 56);
        if (command is 1 or 4)
        {
            var offset = Marshal.ReadInt32(buffer, 16);
            var length = Marshal.ReadInt32(buffer, 20);
            if (offset < 64 || offset > sizeBytes || length <= 0 || length > sizeBytes - offset)
                throw new ArgumentException("Invalid import path range");
            var bytes = new byte[length];
            Marshal.Copy(buffer + offset, bytes, 0, length);
            var path = new UTF8Encoding(false, true).GetString(bytes);
            if (path.Contains('\0')) throw new ArgumentException("Import path contains NUL");
            if (Marshal.ReadInt64(buffer, 40) != 0 || Marshal.ReadInt64(buffer, 48) != 0 || Marshal.ReadInt32(buffer, 60) != 0)
                throw new ArgumentException("Nonzero reserved import fields");
            Aftercast.ActCompat.ImportRange? range = null;
            if (command == 4)
                range = new(new DateTime(Marshal.ReadInt64(buffer, 24), DateTimeKind.Unspecified),
                    new DateTime(Marshal.ReadInt64(buffer, 32), DateTimeKind.Unspecified));
            else if (Marshal.ReadInt64(buffer, 24) != 0 || Marshal.ReadInt64(buffer, 32) != 0)
                throw new ArgumentException("Full import must not specify a time range");
            plugins.Value.BeginImport(path, range);
            return 1;
        }
        if (sizeBytes != 64 || command is not (2 or 3)) throw new ArgumentException("Invalid import step");
        Aftercast.ActCompat.ImportResult? result;
        try { result = plugins.Value.AdvanceImport(command == 3); }
        catch (OperationCanceledException) { return 2; }
        if (result is null)
        {
            var progress = plugins.Value.ImportProgress;
            Marshal.WriteInt64(buffer, 24, progress.LinesRead);
            Marshal.WriteInt64(buffer, 32, progress.RecordsDispatched);
            Marshal.WriteInt64(buffer, 40, progress.BytesRead);
            Marshal.WriteInt64(buffer, 48, progress.TotalBytes);
            return 1;
        }
        Marshal.WriteInt64(buffer, 24, result.LinesRead);
        Marshal.WriteInt64(buffer, 32, result.RecordsDispatched);
        Marshal.WriteInt32(buffer, 40, result.Encounters);
        Marshal.WriteInt32(buffer, 44, result.Combatants);
        Marshal.WriteInt64(buffer, 48, result.Damage);
        Marshal.WriteInt64(buffer, 56, result.Healed);
        return 0;
    });
}
