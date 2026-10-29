using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using Aftercast.ActCompat;

namespace Aftercast.Bridge;

public static partial class EntryPoint
{
    public static int TableCommand(nint buffer, int sizeBytes) => Run(() =>
    {
        const int header = 32;
        if (buffer == 0 || sizeBytes < header) throw new ArgumentException("Invalid table capacity");
        var command = Marshal.ReadInt32(buffer, 16);
        var capacity = command switch { 1 => 4 * 1024 * 1024, 2 or 3 => 16 * 1024 * 1024, _ => throw new ArgumentException("Invalid table command") };
        // Reserve the entire reply before callbacks: no capacity-probing retry may execute plugin code twice.
        if (sizeBytes != header + capacity) throw new ArgumentException("Invalid table capacity");
        ValidateHeader(buffer, sizeBytes, "Table ABI size/version mismatch");
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var length = Marshal.ReadInt32(buffer, 20);
        if (length is < 2 or > 256 * 1024 || Marshal.ReadInt64(buffer, 24) != 0)
            throw new ArgumentException("Invalid table input length or reserved fields");
        var input = new byte[length]; Marshal.Copy(buffer + header, input, 0, length);
        var request = JsonSerializer.Deserialize<TableRequest>(new UTF8Encoding(false, true).GetString(input), HostJson.Options)
            ?? throw new ArgumentException("Missing table request");
        if (command == 1 ? request.Level is null || request.Query is not null : request.Query is null || request.Level is not null)
            throw new ArgumentException("Table request does not match command");
        if (command == 3 ? request.LookupRevision is null or 0 : request.LookupRevision is not null || request.Peer is not null || request.Skill is not null)
            throw new ArgumentException("Invalid lookup table request");
        var output = plugins.Value.TableOperation(request, capacity);
        if (output.Length > capacity) throw new InvalidDataException("Table output exceeds fixed capacity");
        Marshal.Copy(output, 0, buffer + header, output.Length);
        Marshal.WriteInt32(buffer, 24, output.Length);
        return 0;
    });
}
internal sealed record TableRequest(TableLevel? Level = null, TableQuery? Query = null, ulong? LookupRevision = null, string? Peer = null, string? Skill = null);
internal static class TableEncoding
{
    internal static byte[] Encode(HostEnvironment host, TableRequest request, int capacity)
    {
        // Query exactly once, then encode owned data. Encoding never invokes plugin callbacks.
        var catalog = request.Level is { } level ? host.ReadTableColumns(level) : null;
        var page = request.Query is { } query ? request.LookupRevision is { } revision ? host.ReadLookupTable(revision, query, request.Peer, request.Skill) : host.ReadTable(query) : null;
        using var stream = new MemoryStream();
        using var writer = new BinaryWriter(stream, new UTF8Encoding(false, true), true);
        void Budget(long count)
        {
            if (count < 0 || stream.Length + count > capacity) throw new InvalidDataException("Table encoding exceeds capacity");
        }
        void Number(uint value) { Budget(4); writer.Write(value); }
        void Wide(ulong value) { Budget(8); writer.Write(value); }
        void Text(string? text)
        {
            if (text is null) { Number(uint.MaxValue); return; }
            var bytes = Encoding.UTF8.GetByteCount(text); Budget(4L + bytes);
            writer.Write((uint)bytes); writer.Write(text.AsSpan());
        }
        void Optional(int? value) { Number(value.HasValue ? 1u : 0u); if (value.HasValue) Number(unchecked((uint)value.Value)); }
        void Columns(TableColumnData[] columns)
        {
            Number((uint)columns.Length);
            foreach (var column in columns)
            {
                Text(column.Key); Text(column.Label); Text(column.SqlDataType); Text(column.SqlDataName);
                Number((column.DefaultVisible ? 1u : 0u) | (column.CanRead ? 2u : 0u) | (column.CanSort ? 4u : 0u));
            }
        }
        if (catalog is not null)
        {
            Number((uint)catalog.Level); Wide(catalog.Revision); Columns(catalog.Columns);
            Number((uint)catalog.Categories.Length);
            foreach (var category in catalog.Categories)
            {
                Text(category.Key); Text(category.Label); Number(category.Outgoing ? 1u : 0u);
                Number(unchecked((uint)category.AllyValue)); Number(unchecked((uint)category.ColorArgb));
            }
        }
        else
        {
            Number(page is null ? 0u : 1u);
            if (page is not null)
            {
                Number((uint)request.Query!.Scope.Level); Wide(page.Revision); Number((uint)page.Total);
                Columns(page.Columns); Text(page.SortError); Number((uint)page.Rows.Length);
                foreach (var row in page.Rows)
                {
                    Number((uint)row.Index); Wide(row.EncounterId); Text(row.Combatant); Text(row.Category); Text(row.Attack);
                    Optional(row.TimeSorter); Number(row.IsAlly ? 1u : 0u); Number(row.IsSelf ? 1u : 0u); Number((uint)row.Cells.Length);
                    foreach (var cell in row.Cells)
                    {
                        Text(cell.Text); Optional(cell.ForegroundArgb); Optional(cell.BackgroundArgb); Text(cell.Error);
                    }
                }
            }
        }
        return stream.ToArray();
    }
}
