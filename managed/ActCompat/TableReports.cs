using System.Diagnostics;
using System.Net;
using System.Text;
using System.Xml;

namespace Aftercast.ActCompat
{
    public enum TableReportFormat { TableXml, StaticHtml }
    // Index belongs to this catalog snapshot, never a durable action/log identity.
    public sealed record TableReportRow(int Index, ulong EncounterId, string? Combatant, string? Category, string? Attack, int? TimeSorter);
    public sealed record TableReportCatalog(ulong Revision, TableScope Scope, TableColumnData[] Columns, TableReportRow[] Rows);
    public sealed record TableReportOptions(TableReportFormat Format, string Name, int[]? SelectedRows = null,
        string[]? Columns = null, TableSort? Sort = null);
    public sealed record TableReport(ulong Revision, TableScope Scope, TableReportFormat Format, int Rows, string Content);
}
namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;
    public partial class FormActMain
    {
        private sealed record ReportCapture(ulong Revision, TableScope Scope, ulong ColumnRevision, ColumnSnapshot Columns,
            TableSource[] Rows, TextReportScope Guard, object[] Parents);
        private ReportCapture? tableReportCapture;
        private ulong tableReportSequence;
        private object[]? ReportParents(TableScope scope)
        {
            var encounter=TableEncounter(scope.EncounterId);if(encounter is null)return null;
            var parents=new List<object>{encounter,encounter.Items};
            if(scope.Level==TableLevel.Combatant)return parents.ToArray();
            if(!encounter.Items.TryGetValue(scope.Combatant!,out var actor)&&!encounter.Items.TryGetValue(scope.Combatant!.ToUpper(),out actor))return null;
            parents.Add(actor);parents.Add(actor.Items);
            if(scope.Level==TableLevel.Category)return parents.ToArray();
            if(!actor.Items.TryGetValue(scope.Category!,out var category))return null;
            parents.Add(category);parents.Add(category.Items);
            if(scope.Level==TableLevel.Attack)return parents.ToArray();
            if(!category.Items.TryGetValue(scope.Attack!,out var attack))return null;
            parents.Add(attack);parents.Add(attack.Items);return parents.ToArray();
        }
        internal TableReportCatalog? ReadTableReportCatalog(TableScope scope)
        {
            EnsureTextReportOwner();ValidateTableScope(scope);
            if(scope.Level==TableLevel.Encounter)throw new ArgumentException("Current-table report supports combatant/category/attack/action scopes; hierarchical reports are separate");
            lock(CombatGate)
            {
                // One bounded current selection per Host; a new capture retires all earlier row indices.
                tableReportCapture=null;
                var capture=CaptureReportTable(scope);if(capture is null)return null;
                tableReportCapture=capture;
                var rows=capture.Rows;var catalog=capture.Columns.Bindings.Select(binding=>binding.Metadata).ToArray();
                return new(capture.Revision,scope,catalog,rows.Select((row,index)=>new TableReportRow(index,row.EncounterId,row.Combatant,row.Category,row.Attack,row.TimeSorter)).ToArray());
            }
        }
        private ReportCapture? CaptureReportTable(TableScope scope)
        {
            ValidateTableScope(scope);
            var guard=FindTextReportScope(scope.EncounterId);var parents=ReportParents(scope);var source=TableSources(scope);
            if(guard is null||parents is null||source is null)return null;
            var rows=source.Take(MaximumTableRows+1).ToArray();
            if(rows.Length>MaximumTableRows||rows.Any(row=>row.Value is null))throw new InvalidDataException("Invalid or oversized report rows");
            var catalog=ReadTableColumns(scope.Level);var columns=columnSnapshots[scope.Level].Snapshot;
            var capture=new ReportCapture(checked(++tableReportSequence),scope,catalog.Revision,columns,rows,guard,parents);
            CheckTableReport(capture);return capture;
        }
        private void CheckTableReport(ReportCapture capture)
        {
            capture.Guard.Check();RequireTableRevision(capture.Scope.Level,capture.ColumnRevision);
            var parents=ReportParents(capture.Scope);
            if(parents is null||!capture.Parents.SequenceEqual(parents,ReferenceEqualityComparer.Instance))throw new InvalidOperationException("Report parent scope changed");
            var rows=TableSources(capture.Scope)?.Take(MaximumTableRows+1).ToArray();
            if(rows is null||rows.Length!=capture.Rows.Length||!capture.Rows.Zip(rows).All(pair=>
                ReferenceEquals(pair.First.Value,pair.Second.Value)&&pair.First.EncounterId==pair.Second.EncounterId&&
                pair.First.Combatant==pair.Second.Combatant&&pair.First.Category==pair.Second.Category&&pair.First.Attack==pair.Second.Attack&&pair.First.TimeSorter==pair.Second.TimeSorter))throw new InvalidOperationException("Report row selection changed");
            ObjectDisposedException.ThrowIf(actionsClosed,this);
        }
        internal TableReport ReadTableReport(ulong revision,TableReportOptions options)
        {
            EnsureTextReportOwner();ArgumentNullException.ThrowIfNull(options);
            if(!Enum.IsDefined(options.Format))throw new ArgumentException("Unsupported report format");
            var name=ColumnText(options.Name);if(options.Format==TableReportFormat.TableXml)XmlConvert.VerifyXmlChars(name);var selected=options.SelectedRows?.ToArray()??[];var keys=options.Columns?.ToArray();
            if(selected.Length>MaximumTableRows||selected.Distinct().Count()!=selected.Length)throw new ArgumentException("Invalid selected report rows");
            lock(CombatGate)
            {
                var capture=tableReportCapture;
                if(capture is null||revision==0||revision!=capture.Revision)throw new InvalidOperationException("Report selection revision mismatch");
                CheckTableReport(capture);
                if(selected.Any(index=>index<0||index>=capture.Rows.Length))throw new ArgumentException("Selected report row outside snapshot");
                if(options.Format==TableReportFormat.StaticHtml&&selected.Length!=0)throw new ArgumentException("Current-table HTML exports the captured scope; XML row selection is separate");
                var columns=ReportColumns(capture,options.Format,keys);
                var sort=options.Sort;
                var primary=sort is null?null:SelectTableColumn(capture.Columns,sort.Primary,true);
                var secondary=sort?.Secondary is null?null:SelectTableColumn(capture.Columns,sort.Secondary,true);
                activeTableQuery=true;
                try{return RenderTableReport(capture,options.Format,name,selected,columns,sort,primary,secondary);}
                finally{activeTableQuery=false;}
            }
        }
        private static ColumnBinding[] ReportColumns(ReportCapture capture,TableReportFormat format,string[]? keys)
            =>ReportColumns(capture.Columns,format,keys);
        private static ColumnBinding[] ReportColumns(ColumnSnapshot snapshot,TableReportFormat format,string[]? keys)
        {
            if(format==TableReportFormat.TableXml)
            {
                if(keys is not null)throw new ArgumentException("ACT table XML uses all SQL columns in registry order");
                foreach(var column in snapshot.Bindings)
                {XmlConvert.VerifyNCName(column.Metadata.SqlDataName);if(column.Sql is null)throw new InvalidDataException("Missing SQL report callback");}
                return snapshot.Bindings;
            }
            if(keys is null||keys.Length is <1 or >MaximumSelectedColumns||keys.Distinct(StringComparer.Ordinal).Count()!=keys.Length)
                throw new ArgumentException("HTML report requires explicit selected column keys");
            var selected=keys.Select(key=>SelectTableColumn(snapshot,key)).ToArray();
            if(selected.Any(column=>column.Cell is null))throw new InvalidDataException("Missing HTML cell callback");return selected;
        }
        private static string SqlReportCell(ColumnBinding column,object row)=>column.Sql switch
        {
            CombatantData.StringDataCallback callback=>callback((CombatantData)row),
            DamageTypeData.StringDataCallback callback=>callback((DamageTypeData)row),
            AttackType.StringDataCallback callback=>callback((AttackType)row),
            MasterSwing.StringDataCallback callback=>callback((MasterSwing)row),
            _=>throw new InvalidDataException("Invalid SQL report callback")
        };
        private TableReport RenderTableReport(ReportCapture capture,TableReportFormat format,string name,int[] selected,
            ColumnBinding[] columns,TableSort? sort,ColumnBinding? primary,ColumnBinding? secondary)
        {
            var values=ReadReportValues(capture,format,selected,columns,sort,primary,secondary);
            var content=TableReportSerialization.Serialize(format,capture.Scope.Level,name,values.Columns,values.Rows);
            values.Check();return new(capture.Revision,capture.Scope,format,values.Rows.Length,content);
        }
        private sealed record ReportValues(TableColumnData[] Columns,string[][] Rows,Action Check);
        private ReportValues ReadReportValues(ReportCapture capture,TableReportFormat format,int[] selected,
            ColumnBinding[] columns,TableSort? sort,ColumnBinding? primary,ColumnBinding? secondary,
            Action? outerCheck=null,string? firstActionValue=null,bool validateXml=true)
        {
            var clock=Stopwatch.StartNew();var errors=OwnerErrors.Item1;var level=capture.Scope.Level;
            void Check()
            {
                if(clock.Elapsed>TimeSpan.FromSeconds(2))throw new TimeoutException("Table report exceeded cooperative two-second budget");
                outerCheck?.Invoke();CheckTableReport(capture);
                if(OwnerErrors.Item1!=errors)throw new InvalidOperationException("Report callback reported an owner-thread exception");
            }
            T Invoke<T>(ColumnBinding column,Func<T> action)
            {
                Check();RequireTableBinding(level,capture.Columns,column);Exception? failure=null;
                try{return action();}catch(Exception error){failure=error;throw;}
                finally
                {
                    try{Check();RequireTableBinding(level,capture.Columns,column);}
                    catch(Exception after)when(failure is not null){throw new AggregateException("Table report callback and validation failed",failure,after);}
                }
            }
            int Compare(int left,int right)
            {
                var result=Invoke(primary!,()=>CompareTableRows(level,capture.Columns,primary!,capture.Rows[left].Value,capture.Rows[right].Value));
                return result!=0||secondary is null?result:Invoke(secondary,()=>CompareTableRows(level,capture.Columns,secondary,capture.Rows[left].Value,capture.Rows[right].Value));
            }
            var order=Enumerable.Range(0,capture.Rows.Length).ToArray();
            if(primary is not null)
            {
                Exception? sortFailure=null;
                order=TableOrdering.Order(order.Length,Compare,sort!.Descending,error=>sortFailure=error,out var error);
                if(error is not null)throw new InvalidOperationException("Report sort failed: "+error,sortFailure);
            }
            // ACT current-table XML restricts rows only when more than one is selected.
            if(selected.Length>1){var included=selected.ToHashSet();order=order.Where(included.Contains).ToArray();}
            var values=new List<string[]>();var bytes=0;
            foreach(var index in order)
            {
                var row=capture.Rows[index];var cells=new List<string>();
                foreach(var column in columns)
                {
                    string value;
                    if(firstActionValue is not null&&level==TableLevel.Action&&ReferenceEquals(column,columns[0]))
                    {
                        // Native evaluates whole ColCollection when any later field exists; retain first-field failures once.
                        if(columns.Length>1)Invoke(column,()=>SqlReportCell(column,row.Value));
                        value=firstActionValue;
                    }
                    else value=Invoke(column,()=>format==TableReportFormat.TableXml?SqlReportCell(column,row.Value):CellText(column,row.Value));
                    if(value is null||value.Contains('\0')||ColumnUtf8.GetByteCount(value)>MaximumCellBytes)throw new InvalidDataException("Invalid or oversized report cell");
                    if(format==TableReportFormat.TableXml&&validateXml)XmlConvert.VerifyXmlChars(value);
                    bytes+=ColumnUtf8.GetByteCount(value);if(bytes>MaximumTableTextBytes)throw new InvalidDataException("Table report text exceeds 8 MiB");
                    cells.Add(value);
                }
                values.Add(cells.ToArray());
            }
            Check();
            return new(columns.Select(column=>column.Metadata).ToArray(),values.ToArray(),Check);
        }
    }
}
namespace Aftercast.ActCompat
{
    internal static class TableReportSerialization
    {
        internal static string Serialize(TableReportFormat format,TableLevel level,string name,TableColumnData[] columns,string[][] rows)
        {
            using var stream=new ReportStream();
            if(format==TableReportFormat.TableXml)
            {
                var root=level switch{TableLevel.Combatant=>"EncounterTable",TableLevel.Category=>"CombatantTable",TableLevel.Attack=>"DamageTypeTable",TableLevel.Action=>"AttackTypeTable",_=>throw new ArgumentException("Unsupported XML table level")};
                // Preserve ACT XmlTextWriter attribute CR/LF semantics; validate names/chars before writing.
                using(var output=new StreamWriter(stream,new UTF8Encoding(false,true),4096,true){NewLine="\r\n"})
                using(var writer=new XmlTextWriter(output){Formatting=Formatting.Indented,Indentation=4,Namespaces=false})
                {
                    writer.WriteStartDocument();writer.WriteStartElement(root);writer.WriteAttributeString("Name",name);
                    foreach(var row in rows)
                    {
                        writer.WriteStartElement("Row");
                        for(var index=0;index<columns.Length;index++){writer.WriteStartElement(columns[index].SqlDataName);writer.WriteString(row[index]);writer.WriteEndElement();}
                        writer.WriteEndElement();
                    }
                    writer.WriteEndElement();writer.WriteEndDocument();
                }
            }
            else
            {
                using var writer=new StreamWriter(stream,new UTF8Encoding(false,true),4096,true);
                string E(string value)=>WebUtility.HtmlEncode(value);
                writer.Write("<!doctype html>\n<html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; base-uri 'none'; form-action 'none'\"><title>");
                writer.Write(E(name));writer.Write("</title></head><body><table><caption>");writer.Write(E(name));writer.Write("</caption><thead><tr>");
                foreach(var column in columns){writer.Write("<th>");writer.Write(E(column.Key));writer.Write("</th>");}
                writer.Write("</tr></thead><tbody>");
                foreach(var row in rows){writer.Write("<tr>");foreach(var cell in row){writer.Write("<td>");writer.Write(E(cell));writer.Write("</td>");}writer.Write("</tr>");}
                writer.Write("</tbody></table></body></html>");writer.Flush();
            }
            return new UTF8Encoding(false,true).GetString(stream.ToArray());
        }
        internal sealed class ReportStream:MemoryStream
        {
            private const int Maximum=16*1024*1024;
            private void Budget(int count){if(count<0||Length+count>Maximum)throw new InvalidDataException("Serialized table report exceeds 16 MiB UTF-8");}
            public override void Write(byte[] buffer,int offset,int count){Budget(count);base.Write(buffer,offset,count);}
            public override void Write(ReadOnlySpan<byte> buffer){Budget(buffer.Length);base.Write(buffer);}
            public override void WriteByte(byte value){Budget(1);base.WriteByte(value);}
        }
    }
}
