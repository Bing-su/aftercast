using System.Diagnostics;
using System.Net;
using System.Text;

namespace Aftercast.ActCompat
{
    // Four table levels, unlike task XML's three depth choices; managed graphs are explicit opt-in.
    public sealed record HierarchyHtmlCatalog(ulong Revision,ulong[] Encounters,TableLevel LastLevel,int Tables,int Rows);
    public sealed record HierarchyHtmlLevel(TableLevel Level,string[] Columns,TableSort? Sort=null);
    public sealed record HierarchyHtmlReport(ulong Revision,ulong[] Encounters,TableLevel LastLevel,int Tables,int Rows,string Content);
}
namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;
    public partial class FormActMain
    {
        private sealed record HtmlRoot(ulong Id,HierarchyNode Node,string Caption);
        private sealed record HtmlCapture(HierarchyHtmlCatalog Catalog,HtmlRoot[] Roots);
        private HtmlCapture? hierarchyHtml;
        private ulong hierarchyHtmlSequence;
        internal HierarchyHtmlCatalog? CaptureHierarchyHtml(ulong[] encounterIds,TableLevel lastLevel)
        {
            EnsureTextReportOwner();ArgumentNullException.ThrowIfNull(encounterIds);
            if(lastLevel is <TableLevel.Combatant or >TableLevel.Action)throw new ArgumentException("HTML hierarchy requires one to four table levels");
            if(encounterIds.Length is <1 or >64)throw new ArgumentException("Select 1..64 distinct encounter IDs");
            var ids=encounterIds.ToArray();
            if(ids.Contains(0UL)||ids.Distinct().Count()!=ids.Length)throw new ArgumentException("Select 1..64 distinct encounter IDs");
            lock(CombatGate)
            {
                activeTableQuery=true;
                try
                {
                    hierarchyHtml=null;var clock=Stopwatch.StartNew();var tables=0;var rows=0;var scanned=0;var errors=OwnerErrors.Item1;
                    var selected=ids.ToHashSet();var roots=new List<HtmlRoot>();var captures=new List<ReportCapture>();
                    void Guard()
                    {
                        if(clock.Elapsed>TimeSpan.FromSeconds(2))throw new TimeoutException("HTML hierarchy capture exceeded two seconds");
                        if(OwnerErrors.Item1!=errors)throw new InvalidOperationException("HTML caption callback reported an owner-thread error");
                    }
                    if(combatZones.Count>16384)throw new InvalidDataException("Report zone scan exceeds 16384 slots");
                    // ACT checked encounter export follows zone/encounter tree order, not click/request order.
                    foreach(var zone in combatZones)
                        foreach(var encounter in zone.Items.Skip(zone.PopulateAll?1:0))
                        {
                            Guard();if(++scanned>16384)throw new InvalidDataException("Report encounter scan exceeds 16384 slots");
                            var id=Identity(encounter).Id;if(!selected.Contains(id))continue;
                            roots.Add(new(id,BuildHierarchy(new(TableLevel.Combatant,id),lastLevel,ref tables,ref rows,captures,Guard),""));
                        }
                    if(roots.Count!=ids.Length)return null;
                    void Check()=>CheckHierarchies(roots.Select(root=>root.Node),Guard);
                    for(var index=0;index<roots.Count;index++)
                    {
                        Check();string caption;
                        try{caption=ColumnText(roots[index].Node.Capture.Guard.Encounter.ToString());}
                        catch(Exception error){try{Check();}catch(Exception after){throw new AggregateException(error,after);}throw;}
                        Check();roots[index]=roots[index] with{Caption=caption};
                    }
                    var catalog=new HierarchyHtmlCatalog(checked(++hierarchyHtmlSequence),roots.Select(root=>root.Id).ToArray(),lastLevel,tables,rows);
                    hierarchyHtml=new(catalog,roots.ToArray());
                    return catalog with{Encounters=catalog.Encounters.ToArray()};
                }
                finally{activeTableQuery=false;}
            }
        }
        internal HierarchyHtmlReport ReadHierarchyHtml(ulong revision,HierarchyHtmlLevel[] levels,HierarchyHtmlGraphOptions? graphs=null)
        {
            EnsureTextReportOwner();ArgumentNullException.ThrowIfNull(levels);
            if(levels.Length is <1 or >4)throw new ArgumentException("Supply one to four HTML levels");
            // Caller-owned option arrays cannot change an admitted render via a callback.
            if(graphs is not null && (graphs.Window is <1 or >120 || string.IsNullOrEmpty(graphs.DistributionColumn) || string.IsNullOrEmpty(graphs.ActionSortColumn) || string.IsNullOrEmpty(graphs.AllLabel) || graphs.WardPrefix is null))throw new ArgumentException("Invalid HTML graph options");
            if(graphs is not null)
            {
                static string[]? CopySelection(string[]? selection)
                {
                    if(selection is null)return null;
                    if(selection.Length>256||selection.Any(string.IsNullOrEmpty)||selection.Distinct(StringComparer.Ordinal).Count()!=selection.Length)throw new ArgumentException("Invalid graph selection");
                    return selection.ToArray();
                }
                var graphLevels=graphs.Levels?.ToArray()??[TableLevel.Combatant,TableLevel.Category,TableLevel.Attack,TableLevel.Action];
                if(graphLevels.Length is <1 or >4 || graphLevels.Distinct().Count()!=graphLevels.Length || graphLevels.Any(level=>level is <TableLevel.Combatant or >TableLevel.Action))throw new ArgumentException("Invalid graph levels");
                graphs=graphs with{SelectedCombatants=CopySelection(graphs.SelectedCombatants),SoloCategories=CopySelection(graphs.SoloCategories),Levels=graphLevels};
            }
            var options=levels.Select(level=>level is null||level.Columns is null||level.Columns.Length is <1 or >MaximumSelectedColumns?throw new ArgumentException("Missing HTML level/columns"):level with{Columns=level.Columns.ToArray()}).ToArray();
            lock(CombatGate)
            {
                var capture=hierarchyHtml;
                if(capture is null||revision==0||capture.Catalog.Revision!=revision)throw new InvalidOperationException("HTML hierarchy revision mismatch");
                if(graphs is not null && capture.Catalog.LastLevel!=TableLevel.Action)throw new ArgumentException("Graph export requires complete action hierarchy capture");
                if(graphs is not null && graphs.Levels!.Any(level=>(int)level>options.Length))throw new ArgumentException("Each graph requires its corresponding table");
                if(graphs is not null && graphs.Levels!.Contains(TableLevel.Attack) && graphs.DistributionColumn=="Resist" && string.IsNullOrEmpty(graphs.WardPrefix))throw new ArgumentException("Resist graph requires a ward prefix");
                if((graphs is null && options.Length!=(int)capture.Catalog.LastLevel)||(graphs is not null && options.Length>(int)capture.Catalog.LastLevel)||!options.Select(level=>level.Level).Order().SequenceEqual(Enumerable.Range(1,options.Length).Select(level=>(TableLevel)level)))
                    throw new ArgumentException("Supply each included HTML level exactly once");
                if(options.Any(level=>level.Level==TableLevel.Category&&level.Sort is not null))throw new ArgumentException("ACT category HTML preserves source order without a sorter");
                // Validate graph callbacks before any table or graph value callback runs.
                if(graphs is not null)
                {
                    if(graphs.Simple && graphs.Levels!.Contains(TableLevel.Combatant))
                    {
                        ReadTableColumns(TableLevel.Combatant);var simple=SelectTableColumn(columnSnapshots[TableLevel.Combatant].Snapshot,graphs.SimpleColumn,true);
                        if(simple.Sql is not CombatantData.StringDataCallback || simple.Sort is not Comparison<CombatantData>)throw new ArgumentException("Simple graph requires SQL and sort callbacks");
                    }
                    ReadTableColumns(TableLevel.Attack);var attackSnapshot=columnSnapshots[TableLevel.Attack].Snapshot;
                    if(graphs.Levels!.Contains(TableLevel.Attack) && graphs.DistributionColumn!="Resist")
                    {
                        var graphColumn=SelectTableColumn(attackSnapshot,graphs.DistributionColumn,true);
                        if(graphColumn.Sql is not AttackType.StringDataCallback || graphColumn.Sort is not Comparison<AttackType>)throw new ArgumentException("Graph distribution requires SQL and sort callbacks");
                    }
                    ReadTableColumns(TableLevel.Action);var actionSnapshot=columnSnapshots[TableLevel.Action].Snapshot;
                    if(graphs.Levels!.Contains(TableLevel.Action) && SelectTableColumn(actionSnapshot,graphs.ActionSortColumn,true).Sort is not Comparison<MasterSwing>)throw new ArgumentException("Graph bars require an action sort callback");
                }
                // Validate explicit settings even when an empty parent has no child table at that level.
                var revisions=new Dictionary<TableLevel,ulong>();
                foreach(var setting in options)
                {
                    revisions.Add(setting.Level,ReadTableColumns(setting.Level).Revision);var snapshot=columnSnapshots[setting.Level].Snapshot;
                    ReportColumns(snapshot,TableReportFormat.StaticHtml,setting.Columns);
                    if(setting.Sort is not null)SelectTableColumn(snapshot,setting.Sort.Primary,true);
                    if(setting.Sort?.Secondary is not null)SelectTableColumn(snapshot,setting.Sort.Secondary,true);
                }
                var byLevel=options.ToDictionary(level=>level.Level);var clock=Stopwatch.StartNew();var errors=OwnerErrors.Item1;var bytes=0;
                var selectedRevision=selectiveRevision;
                void Guard()
                {
                    if(clock.Elapsed>TimeSpan.FromSeconds(4))throw new TimeoutException("HTML hierarchy render exceeded four seconds");
                    if(graphs is not null && selectiveRevision!=selectedRevision)throw new InvalidOperationException("Graph selection changed during rendering");
                    foreach(var setting in revisions)RequireTableRevision(setting.Key,setting.Value);
                    if(OwnerErrors.Item1!=errors)throw new InvalidOperationException("HTML hierarchy callback reported an owner-thread error");
                }
                void Check()=>CheckHierarchies(capture.Roots.Select(root=>root.Node),Guard);
                activeTableQuery=true;
                try
                {
                    Check();
                    if(graphs?.SelectedCombatants is { } selectedActors)
                    {
                        var actors=capture.Roots.SelectMany(root=>root.Node.Capture.Rows).ToArray();
                        if(selectedActors.Any(key=>!actors.Any(row=>row.Combatant==key)))throw new InvalidOperationException("Selected graph combatant no longer exists");
                        var names=actors.Where(row=>selectedActors.Contains(row.Combatant,StringComparer.Ordinal)).Select(row=>((CombatantData)row.Value).Name.ToUpper()).Distinct(StringComparer.Ordinal).ToArray();
                        graphs=graphs with{SelectedCombatants=names};
                    }
                    if(graphs?.SoloCategories is { } selectedCategories)
                    {
                        ReadTableColumns(TableLevel.Category);var available=columnSnapshots[TableLevel.Category].Snapshot.Categories;
                        if(selectedCategories.Any(key=>!available.Any(category=>category.Key==key)))throw new InvalidOperationException("Selected graph category no longer exists");
                    }
                    Check();
                    var nodes=capture.Roots.SelectMany(root=>HierarchyNodes(root.Node)).ToArray();
                    // Validate every level/key/sorter before any display callback runs.
                    IEqualityComparer<HierarchyNode> comparer=ReferenceEqualityComparer.Instance;
                    var bindings=nodes.Where(node=>byLevel.ContainsKey(node.Capture.Scope.Level)).ToDictionary(node=>node,node=>
                    {
                        var setting=byLevel[node.Capture.Scope.Level];var columns=ReportColumns(node.Capture,TableReportFormat.StaticHtml,setting.Columns);
                        var primary=setting.Sort is null?null:SelectTableColumn(node.Capture.Columns,setting.Sort.Primary,true);
                        var secondary=setting.Sort?.Secondary is null?null:SelectTableColumn(node.Capture.Columns,setting.Sort.Secondary,true);
                        return (columns,setting.Sort,primary,secondary);
                    },comparer);
                    var values=new Dictionary<HierarchyNode,ReportValues>(comparer);
                    foreach(var node in nodes.Where(node=>bindings.ContainsKey(node)))
                    {
                        var setting=bindings[node];
                        var value=ReadReportValues(node.Capture,TableReportFormat.StaticHtml,[],setting.columns,setting.Sort,setting.primary,setting.secondary,Check);
                        foreach(var cell in value.Rows.SelectMany(row=>row))
                        {bytes=checked(bytes+ColumnUtf8.GetByteCount(cell));if(bytes>8*1024*1024)throw new InvalidDataException("Hierarchy HTML text exceeds 8 MiB");}
                        values.Add(node,value);
                    }
                    using var stream=new TableReportSerialization.ReportStream();
                    using(var writer=new StreamWriter(stream,new UTF8Encoding(false,true),4096,true))
                    {
                        string E(string value)=>WebUtility.HtmlEncode(value);
                        writer.Write("<!doctype html>\n<html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; base-uri 'none'; form-action 'none'\"><title>ACT encounter hierarchy</title></head><body>");
                        var ordinal=0;
                        foreach(var root in capture.Roots)
                        {
                            void Write(HierarchyNode node)
                            {
                                if((int)node.Capture.Scope.Level>options.Length)return;
                                Check();var name=string.Join(" | ",new[]{root.Caption}.Concat(node.Names));
                                writer.Write("<section id=\"h");writer.Write(ordinal++);writer.Write("\"><h2>");writer.Write(E(name));writer.Write("</h2>");
                                if(values.TryGetValue(node,out var data))
                                {
                                    writer.Write("<table><caption>");writer.Write(E(name));writer.Write("</caption><thead><tr>");
                                    foreach(var column in data.Columns){writer.Write("<th>");writer.Write(E(column.Key));writer.Write("</th>");}
                                    writer.Write("</tr></thead><tbody>");
                                    foreach(var row in data.Rows){writer.Write("<tr>");foreach(var cell in row){writer.Write("<td>");writer.Write(E(cell));writer.Write("</td>");}writer.Write("</tr>");}
                                    writer.Write("</tbody></table>");
                                }
                                if(graphs?.Levels?.Contains(node.Capture.Scope.Level)==true)foreach(var graph in ReadHierarchyGraphs(node,graphs,Check)){Check();writer.Write(graph);Check();}
                                // Child navigation order is source order; an individual table sort does not retarget children.
                                foreach(var child in node.Children.SelectMany(children=>children))Write(child);
                                writer.Write("</section>");
                            }
                            Write(root.Node);
                        }
                        writer.Write("</body></html>");writer.Flush();
                    }
                    Check();var catalog=capture.Catalog;
                    return new(catalog.Revision,catalog.Encounters.ToArray(),catalog.LastLevel,catalog.Tables,catalog.Rows,new UTF8Encoding(false,true).GetString(stream.ToArray()));
                }
                finally{activeTableQuery=false;}
            }
        }
    }
}
