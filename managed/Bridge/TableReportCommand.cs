using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using Aftercast.ActCompat;

namespace Aftercast.Bridge;

public static partial class EntryPoint
{
    private static bool ValidHtmlLevels(HierarchyHtmlLevel[] levels)
    {
        bool Key(string? key)=>!string.IsNullOrEmpty(key)&&!key.Contains('\0')&&new UTF8Encoding(false,true).GetByteCount(key)<=1024;
        return levels.Length is >=1 and <=4 && levels.Select(level=>level?.Level).Distinct().Count()==levels.Length && levels.All(level=>
            level is not null && level.Level is >=TableLevel.Combatant and <=TableLevel.Action && level.Columns is {Length:>=1 and <=32} columns &&
            columns.All(Key) && columns.Distinct(StringComparer.Ordinal).Count()==columns.Length &&
            (level.Sort is null || level.Level!=TableLevel.Category && Key(level.Sort.Primary) && (level.Sort.Secondary is null || Key(level.Sort.Secondary))));
    }
    public static int TableReportCommand(nint buffer, int sizeBytes) => Run(() =>
    {
        const int header = 32;
        if (buffer == 0 || sizeBytes < header) throw new ArgumentException("Invalid table report buffer");
        var command = Marshal.ReadInt32(buffer, 16);
        var capacity = command switch { 1 => 56 * 1024 * 1024, 2 => 16 * 1024 * 1024 + 20, 3 => 4096, 4 => 16 * 1024 * 1024 + 32, 5 => 4096, 6 => 16 * 1024 * 1024 + 1024, _ => throw new ArgumentException("Invalid table report command") };
        // Reserve the bounded binary reply before callbacks; UTF-8 content needs no JSON re-escaping or retry.
        if (sizeBytes != header + capacity) throw new ArgumentException("Invalid table report capacity");
        ValidateHeader(buffer, sizeBytes, "Table report ABI size/version mismatch");
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var length = Marshal.ReadInt32(buffer, 20);
        if (length is < 2 or > 256 * 1024 || length > capacity || Marshal.ReadInt64(buffer, 24) != 0)
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
        if(command <= 4 && (request.Html is not null || request.HtmlLevels is not null))throw new ArgumentException("Unexpected HTML hierarchy fields");
        var valid = command switch
        {
            1 => request.Scope is not null && request.Revision == 0 && request.Options is null && request.Hierarchy is null,
            2 => request.Scope is null && request.Revision != 0 && request.Options is not null && request.Hierarchy is null,
            3 => request.Scope is null && request.Revision == 0 && request.Options is null && request.Hierarchy is {} hierarchy && hierarchy.EncounterId != 0 && Enum.IsDefined(hierarchy.Depth),
            4 => request.Scope is null && request.Revision != 0 && request.Options is null && request.Hierarchy is null,
            5 => request.Scope is null && request.Revision == 0 && request.Options is null && request.Hierarchy is null && request.HtmlLevels is null && request.Html is {} html && html.Encounters is {Length:>=1 and <=64} ids && ids.All(id=>id!=0) && ids.Distinct().Count()==ids.Length && html.LastLevel is >=TableLevel.Combatant and <=TableLevel.Action,
            6 => request.Scope is null && request.Revision != 0 && request.Options is null && request.Hierarchy is null && request.Html is null && request.HtmlLevels is {} levels && ValidHtmlLevels(levels),
            _ => false
        };
        if (!valid) throw new ArgumentException("Table report request does not match command");
        byte[] output;
        try { output = command <= 2 ? plugins.Value.TableReportOperation(request, capacity) : command <= 4 ? plugins.Value.HierarchyXmlOperation(command, request) : plugins.Value.HierarchyHtmlOperation(command,request); }
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
internal sealed record TableReportRequest(TableScope? Scope = null, ulong Revision = 0, TableReportOptions? Options = null, HierarchyXmlRequest? Hierarchy = null, HierarchyHtmlRequest? Html = null, HierarchyHtmlLevel[]? HtmlLevels = null);
internal sealed record HierarchyHtmlRequest(ulong[] Encounters,TableLevel LastLevel);
internal sealed record HierarchyXmlRequest(ulong EncounterId, HierarchyXmlDepth Depth);
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

internal static class HierarchyXmlEncoding
{
    internal static byte[] Encode(HostEnvironment host,int command,TableReportRequest request)
    {
        // Fixed reply capacity was reserved before owner callbacks; UTF-8 is transmitted without JSON escaping.
        var catalog=command==3?host.CaptureHierarchyXml(request.Hierarchy!.EncounterId,request.Hierarchy.Depth):null;
        var report=command==4?host.ReadHierarchyXml(request.Revision):null;
        using var stream=new MemoryStream();using var writer=new BinaryWriter(stream,new UTF8Encoding(false,true),true);
        if(command==3)writer.Write(catalog is null?0u:1u);
        if(catalog is not null||report is not null)
        {
            writer.Write(catalog?.Revision??report!.Revision);writer.Write(catalog?.EncounterId??report!.EncounterId);
            writer.Write((uint)(catalog?.Depth??report!.Depth));writer.Write((uint)(catalog?.Tables??report!.Tables));writer.Write((uint)(catalog?.Rows??report!.Rows));
            if(report is not null)
            {
                var count=new UTF8Encoding(false,true).GetByteCount(report.Content);
                if(count>16*1024*1024)throw new InvalidDataException("Hierarchy XML reply exceeds 16 MiB");
                writer.Write((uint)count);writer.Write(report.Content.AsSpan());
            }
        }
        return stream.ToArray();
    }
}

internal static class HierarchyHtmlEncoding
{
    internal static byte[] Encode(HostEnvironment host,int command,TableReportRequest request)
    {
        // Original owner callbacks execute once; only owned primitive metadata/source is serialized.
        var catalog=command==5?host.CaptureHierarchyHtml(request.Html!.Encounters,request.Html.LastLevel):null;
        var report=command==6?host.ReadHierarchyHtml(request.Revision,request.HtmlLevels!):null;
        using var stream=new MemoryStream();using var writer=new BinaryWriter(stream,new UTF8Encoding(false,true),true);
        if(command==5)writer.Write(catalog is null?0u:1u);
        if(catalog is not null||report is not null)
        {
            var ids=catalog?.Encounters??report!.Encounters;
            if(ids.Length is <1 or >64)throw new InvalidDataException("Invalid HTML hierarchy encounter count");
            writer.Write(catalog?.Revision??report!.Revision);writer.Write((uint)(catalog?.LastLevel??report!.LastLevel));
            writer.Write((uint)(catalog?.Tables??report!.Tables));writer.Write((uint)(catalog?.Rows??report!.Rows));
            writer.Write((uint)ids.Length);foreach(var id in ids)writer.Write(id);
            if(report is not null)
            {
                var count=new UTF8Encoding(false,true).GetByteCount(report.Content);
                if(count>16*1024*1024)throw new InvalidDataException("Hierarchy HTML reply exceeds 16 MiB");
                writer.Write((uint)count);writer.Write(report.Content.AsSpan());
            }
        }
        return stream.ToArray();
    }
}
