using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using Aftercast.ActCompat;

namespace Aftercast.Bridge;

public static partial class EntryPoint
{
    public static int CompanionCommand(nint buffer, int sizeBytes) => Run(() =>
    {
        const int header = 32, capacity = 512 * 1024;
        // Fixed capacity precedes any parse callback/open/close. A short-buffer retry cannot repeat mutations.
        if (buffer == 0 || sizeBytes != header + capacity) throw new ArgumentException("Invalid companion packet capacity");
        ValidateHeader(buffer, sizeBytes, "Companion ABI size/version mismatch");
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var command = Marshal.ReadInt32(buffer, 16);
        var length = Marshal.ReadInt32(buffer, 20);
        if (command is < 1 or > 7 || length is < 2 or > 128 * 1024 || Marshal.ReadInt64(buffer, 24) != 0)
            throw new ArgumentException("Invalid companion command fields");
        var bytes = new byte[length]; Marshal.Copy(buffer + header, bytes, 0, length);
        var json = new UTF8Encoding(false, true).GetString(bytes);
        using (var document = JsonDocument.Parse(json, new JsonDocumentOptions { MaxDepth = 16 }))
        {
            if (document.RootElement.ValueKind != JsonValueKind.Object) throw new ArgumentException("Companion request must be an object");
            Unique(document.RootElement);
        }
        var request = JsonSerializer.Deserialize<CompanionRequest>(json, HostJson.Options) ?? throw new ArgumentException("Missing companion request");
        request.Validate(command);
        var output = plugins.Value.CompanionOperation(command, request);
        if (output.Length > capacity) throw new InvalidDataException("Companion output exceeds bounded capacity");
        Marshal.Copy(output, 0, buffer + header, output.Length); Marshal.WriteInt32(buffer, 24, output.Length);
        return 0;
    });
    private static void Unique(JsonElement value)
    {
        if (value.ValueKind == JsonValueKind.Object)
        {
            var keys = new HashSet<string>(StringComparer.Ordinal);
            foreach (var property in value.EnumerateObject())
            {
                if (!keys.Add(property.Name)) throw new ArgumentException("Duplicate companion JSON key");
                Unique(property.Value);
            }
        }
        else if (value.ValueKind == JsonValueKind.Array) foreach (var item in value.EnumerateArray()) Unique(item);
    }
}
internal sealed record CompanionRequest(ulong Owner = 0, string? Path = null, ImportTextOptions? Text = null,
    ImportCompanion[]? Companions = null, long? StartTicks = null, long? EndTicks = null,
    bool? GetCurrentZone = null, int? ClockWaitMilliseconds = null, bool? ExtendToCombatEnd = null)
{
    internal ImportRange? Range => StartTicks is { } start && EndTicks is { } end ? new(new(start), new(end), ExtendToCombatEnd == true) : null;
    internal void Validate(int command)
    {
        if (command is 1 or 4 or 7)
        {
            if (string.IsNullOrWhiteSpace(Path) || !System.IO.Path.IsPathFullyQualified(Path) || Path.Any(char.IsControl) || new UTF8Encoding(false, true).GetByteCount(Path) > 8192)
                throw new ArgumentException("Companion main path must be absolute, control-free and at most 8192 UTF-8 bytes");
            if (command == 7)
            {
                if (Owner != 0 || Text is not null || Companions is not null || StartTicks is not null || EndTicks is not null ||
                    GetCurrentZone is not null || ClockWaitMilliseconds is not null || ExtendToCombatEnd is not null) throw new ArgumentException("Invalid companion catalog fields");
                return;
            }
            if (Companions is null || Companions.Length > (command == 1 ? 64 : 8)) throw new ArgumentException("Invalid companion selection count");
            Text?.Validate();
            var names = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            foreach (var source in Companions)
            {
                ArgumentNullException.ThrowIfNull(source); source.Validate();
                if (!names.Add(source.Name)) throw new ArgumentException("Duplicate companion selection");
            }
            if (command == 1)
            {
                if (Owner != 0 || GetCurrentZone is not null || ClockWaitMilliseconds is not null || StartTicks.HasValue != EndTicks.HasValue || ExtendToCombatEnd is not null && StartTicks is null)
                    throw new ArgumentException("Invalid source import fields");
                _ = Range;
            }
            else if (StartTicks is not null || EndTicks is not null || ExtendToCombatEnd is not null || ClockWaitMilliseconds is <= 0 or > 300000)
                throw new ArgumentException("Invalid live companion options");
        }
        else if (Owner == 0 || Path is not null || Text is not null || Companions is not null || StartTicks is not null || EndTicks is not null ||
            GetCurrentZone is not null || ClockWaitMilliseconds is not null || ExtendToCombatEnd is not null) throw new ArgumentException("Invalid companion continuation fields");
    }
}
internal sealed record SourceImportReply(ulong Owner, bool Pending = false, bool Cancelled = false, ImportProgress? Progress = null, ImportResult? Result = null);
internal sealed record LiveSourceReply(string SelectedName, string Name, string Path, string State, int Queued, long? PendingTicks,
    long Generation, long Dropped, string LastError, bool ErrorTruncated);
internal sealed record LiveSelectionReply(ulong Owner, string Path, ImportTextOptions? Text, ImportCompanion[] Companions,
    bool GetCurrentZone, int ClockWaitMilliseconds, LiveSourceReply[] Sources);
