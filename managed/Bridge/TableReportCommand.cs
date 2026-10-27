using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using Aftercast.ActCompat;

namespace Aftercast.Bridge;

public static partial class EntryPoint
{
    public static int TableReportCommand(nint buffer, int sizeBytes) => Run(() =>
    {
        const int header = 32;
        if (buffer == 0 || sizeBytes < header) throw new ArgumentException("Invalid table report buffer");
        var command = Marshal.ReadInt32(buffer, 16);
        var capacity = command switch { 1 => 56 * 1024 * 1024, 2 => 16 * 1024 * 1024 + 20, _ => throw new ArgumentException("Invalid table report command") };
        // Reserve the bounded binary reply before callbacks; UTF-8 content needs no JSON re-escaping or retry.
        if (sizeBytes != header + capacity) throw new ArgumentException("Invalid table report capacity");
        ValidateHeader(buffer, sizeBytes, "Table report ABI size/version mismatch");
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var length = Marshal.ReadInt32(buffer, 20);
        if (length is < 2 or > 256 * 1024 || Marshal.ReadInt64(buffer, 24) != 0)
            throw new ArgumentException("Invalid table report request fields");
        var input = new byte[length]; Marshal.Copy(buffer + header, input, 0, length);
        var json = new UTF8Encoding(false, true).GetString(input);
        using (var document = JsonDocument.Parse(json, new JsonDocumentOptions { MaxDepth = 16 }))
        {
            if (document.RootElement.ValueKind != JsonValueKind.Object) throw new ArgumentException("Table report request must be an object");
            Unique(document.RootElement);
        }
        var request = JsonSerializer.Deserialize<TableReportRequest>(json, HostJson.Options)
            ?? throw new ArgumentException("Missing table report request");
        if (command == 1 ? request.Scope is null || request.Revision != 0 || request.Options is not null :
            request.Scope is not null || request.Revision == 0 || request.Options is null)
            throw new ArgumentException("Table report request does not match command");
        byte[] output;
        try { output = plugins.Value.TableReportOperation(request, capacity); }
        catch (Exception error) when (error.InnerException is not null)
        {
            // Preserve bounded callback causes across the native boundary; Describe tolerates hostile Message getters.
            var pending = new Queue<Exception>(); pending.Enqueue(error);
            var causes = new List<string>();
            while (pending.Count != 0 && causes.Count < 4)
            {
                var current = pending.Dequeue(); causes.Add(Describe(current));
                if (current is AggregateException aggregate) foreach (var cause in aggregate.InnerExceptions.Take(4)) pending.Enqueue(cause);
                else if (current.InnerException is {} cause) pending.Enqueue(cause);
            }
            var diagnostic = string.Join(" -> ", causes);
            throw new InvalidOperationException(diagnostic.Length > 1900 ? diagnostic[..1900] : diagnostic, error);
        }
        Marshal.Copy(output, 0, buffer + header, output.Length);
        Marshal.WriteInt32(buffer, 24, output.Length);
        return 0;
    });
}
internal sealed record TableReportRequest(TableScope? Scope = null, ulong Revision = 0, TableReportOptions? Options = null);
internal static class TableReportEncoding
{
    internal static byte[] Encode(HostEnvironment host, TableReportRequest request, int capacity)
    {
        // Invoke the owner operation once. Binary serialization only visits owned records/strings.
        var catalog = request.Scope is {} scope ? host.ReadTableReportCatalog(scope) : null;
        var report = request.Options is {} options ? host.ReadTableReport(request.Revision, options) : null;
        using var stream = new MemoryStream();
        using var writer = new BinaryWriter(stream, new UTF8Encoding(false, true), true);
        void Budget(long count) { if (count < 0 || stream.Length + count > capacity) throw new InvalidDataException("Table report reply exceeds capacity"); }
        void Number(uint value) { Budget(4); writer.Write(value); }
        void Wide(ulong value) { Budget(8); writer.Write(value); }
        void Text(string? value)
        {
            if (value is null) { Number(uint.MaxValue); return; }
            Budget(4L + Encoding.UTF8.GetByteCount(value)); writer.Write((uint)Encoding.UTF8.GetByteCount(value)); writer.Write(value.AsSpan());
        }
        if (request.Scope is not null)
        {
            Number(catalog is null ? 0u : 1u);
            if (catalog is not null)
            {
                Wide(catalog.Revision); Number((uint)catalog.Scope.Level); Wide(catalog.Scope.EncounterId);
                Text(catalog.Scope.Combatant); Text(catalog.Scope.Category); Text(catalog.Scope.Attack);
                Number((uint)catalog.Columns.Length);
                foreach (var column in catalog.Columns)
                {
                    Text(column.Key); Text(column.Label); Text(column.SqlDataType); Text(column.SqlDataName);
                    Number((column.DefaultVisible ? 1u : 0u) | (column.CanRead ? 2u : 0u) | (column.CanSort ? 4u : 0u));
                }
                Number((uint)catalog.Rows.Length);
                foreach (var row in catalog.Rows)
                {
                    Number((uint)row.Index); Wide(row.EncounterId); Text(row.Combatant); Text(row.Category); Text(row.Attack);
                    Number(row.TimeSorter.HasValue ? 1u : 0u); if (row.TimeSorter is {} sorter) Number(unchecked((uint)sorter));
                }
            }
        }
        else
        {
            Wide(report!.Revision); Number((uint)report.Format); Number((uint)report.Rows); Text(report.Content);
        }
        return stream.ToArray();
    }
}
