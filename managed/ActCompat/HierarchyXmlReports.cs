using System.Diagnostics;
using System.Text;
using System.Xml;

namespace Aftercast.ActCompat
{
    public enum HierarchyXmlDepth { MainTable=1, DamageTypeTables=2, AllTables=3 }
    public sealed record HierarchyXmlCatalog(ulong Revision,ulong EncounterId,HierarchyXmlDepth Depth,int Tables,int Rows);
    public sealed record HierarchyXmlReport(ulong Revision,ulong EncounterId,HierarchyXmlDepth Depth,int Tables,int Rows,string Content);
}
namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;
    public partial class FormActMain
    {
        private sealed record HierarchyNode(ReportCapture Capture,string[] Names,HierarchyNode[][] Children);
        private sealed record HierarchyCapture(HierarchyXmlCatalog Catalog,HierarchyNode Root,string Caption,string EncId);
        private HierarchyCapture? hierarchyXml;
        private ulong hierarchySequence;
        private static string[] HierarchyNames(ReportCapture capture,Action check)
        {
            check();Exception? failure=null;
            try
            {
                var names=new List<string>();var parents=capture.Parents;
                if(parents.Length>=4)names.Add(ColumnText(((CombatantData)parents[2]).Name));
                if(parents.Length>=6)names.Add(ColumnText(((DamageTypeData)parents[4]).Type));
                if(parents.Length>=8)names.Add(ColumnText(((AttackType)parents[6]).Type));
                foreach(var name in names)XmlConvert.VerifyXmlChars(name);return names.ToArray();
            }
            catch(Exception error){failure=error;throw;}
            finally
            {
                try{check();}
                catch(Exception after)when(failure is not null){throw new AggregateException("Hierarchy name callback and validation failed",failure,after);}
            }
        }
        private static IEnumerable<HierarchyNode> HierarchyNodes(HierarchyNode node)
        {yield return node;foreach(var child in node.Children.SelectMany(children=>children))foreach(var nested in HierarchyNodes(child))yield return nested;}
        private void CheckHierarchy(HierarchyNode root,Action guard)=>CheckHierarchies([root],guard);
        private void CheckHierarchies(IEnumerable<HierarchyNode> roots,Action guard)
        {
            var nodes=roots.SelectMany(root=>HierarchyNodes(root)).ToArray();
            // Check the whole structure around name callbacks, not recursively around each getter.
            void Check(){guard();foreach(var node in nodes)CheckTableReport(node.Capture);}
            Check();Exception? failure=null;
            try
            {
                foreach(var node in nodes)
                {
                    // An earlier name callback may remove this later scope; do not invoke its stale getter.
                    void CheckName(){guard();CheckTableReport(node.Capture);}
                    if(!node.Names.SequenceEqual(HierarchyNames(node.Capture,CheckName),StringComparer.Ordinal))throw new InvalidOperationException("Hierarchy report names changed");
                }
            }
            catch(Exception error){failure=error;throw;}
            finally
            {
                try{Check();}
                catch(Exception after)when(failure is not null){throw new AggregateException("Hierarchy names and structure validation failed",failure,after);}
            }
        }
        // Share topology capture only; XML's three depth choices differ from HTML's four table levels.
        private HierarchyNode BuildHierarchy(TableScope scope,TableLevel last,ref int tables,ref int rows,List<ReportCapture> captures,Action guard)
        {
            guard();
            if(++tables>1024)throw new InvalidDataException("Hierarchy exceeds 1024 tables");
            var capture=CaptureReportTable(scope)??throw new InvalidOperationException("Hierarchy scope disappeared");
            captures.Add(capture);
            rows=checked(rows+capture.Rows.Length);if(rows>16384)throw new InvalidDataException("Hierarchy exceeds 16384 aggregate rows");
            var children=new HierarchyNode[capture.Rows.Length][];
            for(var index=0;index<children.Length;index++)
            {
                var row=capture.Rows[index];
                children[index]=scope.Level<last?[BuildHierarchy(scope.Level switch
                {
                    TableLevel.Combatant=>new(TableLevel.Category,scope.EncounterId,row.Combatant),
                    TableLevel.Category=>new(TableLevel.Attack,scope.EncounterId,row.Combatant,row.Category),
                    TableLevel.Attack=>new(TableLevel.Action,scope.EncounterId,row.Combatant,row.Category,row.Attack),
                    _=>throw new InvalidOperationException("Invalid hierarchy transition")
                },last,ref tables,ref rows,captures,guard)]:[];
            }
            void Check(){guard();CheckTableReport(capture);}
            try{return new(capture,HierarchyNames(capture,Check),children);}
            catch(Exception error)
            {
                // A failing getter may mutate another already captured branch; retain both causes.
                try{guard();foreach(var item in captures)CheckTableReport(item);}
                catch(Exception after){throw new AggregateException("Hierarchy capture name and structure failed",error,after);}
                throw;
            }
        }
        internal HierarchyXmlCatalog? CaptureHierarchyXml(ulong encounterId,HierarchyXmlDepth depth)
        {
            EnsureTextReportOwner();if(!Enum.IsDefined(depth))throw new ArgumentException("Invalid task XML depth");
            lock(CombatGate)
            {
                activeTableQuery=true;
                try
                {
                    hierarchyXml=null;var tables=0;var rows=0;var clock=Stopwatch.StartNew();var errors=OwnerErrors.Item1;
                    void Guard(){if(clock.Elapsed>TimeSpan.FromSeconds(2))throw new TimeoutException("Hierarchy capture exceeded two seconds");if(OwnerErrors.Item1!=errors)throw new InvalidOperationException("Hierarchy caption callback reported an owner-thread error");}
                    if(TableEncounter(encounterId) is null)return null;
                    var last=depth==HierarchyXmlDepth.MainTable?TableLevel.Combatant:depth==HierarchyXmlDepth.DamageTypeTables?TableLevel.Attack:TableLevel.Action;
                    var root=BuildHierarchy(new(TableLevel.Combatant,encounterId),last,ref tables,ref rows,[],Guard);
                    void Check()=>CheckHierarchy(root,Guard);
                    Check();var encounter=root.Capture.Guard.Encounter;string caption,id;
                    try{caption=ColumnText(encounter.ToString());id=ColumnText(encounter.EncId);}
                    catch(Exception error){try{Check();}catch(Exception after){throw new AggregateException(error,after);}throw;}
                    Check();XmlConvert.VerifyXmlChars(caption);XmlConvert.VerifyXmlChars(id);
                    var catalog=new HierarchyXmlCatalog(checked(++hierarchySequence),encounterId,depth,tables,rows);
                    hierarchyXml=new(catalog,root,caption,id);return catalog;
                }
                finally{activeTableQuery=false;}
            }
        }
        internal HierarchyXmlReport ReadHierarchyXml(ulong revision)
        {
            EnsureTextReportOwner();lock(CombatGate)
            {
                var capture=hierarchyXml;
                if(capture is null||revision==0||capture.Catalog.Revision!=revision)throw new InvalidOperationException("Hierarchy revision mismatch");
                var clock=Stopwatch.StartNew();var errors=OwnerErrors.Item1;var bytes=0;
                void Guard()
                {
                    if(clock.Elapsed>TimeSpan.FromSeconds(4))throw new TimeoutException("Hierarchy render exceeded four seconds");
                    if(OwnerErrors.Item1!=errors)throw new InvalidOperationException("Hierarchy callback reported an owner-thread error");
                }
                void Check()=>CheckHierarchy(capture.Root,Guard);
                activeTableQuery=true;
                try
                {
                    Check();
                    var values=new Dictionary<HierarchyNode,ReportValues>(ReferenceEqualityComparer.Instance);
                    foreach(var node in HierarchyNodes(capture.Root))
                    {
                        var columns=ReportColumns(node.Capture,TableReportFormat.TableXml,null);
                        var result=ReadReportValues(node.Capture,TableReportFormat.TableXml,[],columns,null,null,null,Check,capture.EncId);
                        foreach(var value in result.Rows.SelectMany(row=>row))
                        {bytes=checked(bytes+ColumnUtf8.GetByteCount(value));if(bytes>8*1024*1024)throw new InvalidDataException("Hierarchy SQL text exceeds 8 MiB");}
                        values.Add(node,result);
                    }
                    using var stream=new TableReportSerialization.ReportStream();
                    using(var output=new StreamWriter(stream,new UTF8Encoding(false,true),4096,true){NewLine="\r\n"})
                    using(var writer=new XmlTextWriter(output){Formatting=Formatting.Indented,Indentation=4,Namespaces=false})
                    {
                        void Write(HierarchyNode node)
                        {
                            Check();var data=values[node];
                            writer.WriteStartElement(node.Capture.Scope.Level switch{TableLevel.Combatant=>"EncounterTable",TableLevel.Category=>"CombatantTable",TableLevel.Attack=>"DamageTypeTable",TableLevel.Action=>"AttackTypeTable",_=>throw new InvalidDataException("Invalid hierarchy level")});
                            writer.WriteAttributeString("Name",string.Join(" | ",new[]{capture.Caption}.Concat(node.Names)));
                            for(var index=0;index<data.Rows.Length;index++)
                            {
                                Check();writer.WriteStartElement("Row");
                                for(var column=0;column<data.Columns.Length;column++)writer.WriteElementString(data.Columns[column].SqlDataName,data.Rows[index][column]);
                                foreach(var child in node.Children[index])Write(child);
                                writer.WriteEndElement();
                            }
                            writer.WriteEndElement();
                        }
                        writer.WriteStartDocument();Write(capture.Root);writer.WriteEndDocument();
                    }
                    Check();var catalog=capture.Catalog;
                    return new(catalog.Revision,catalog.EncounterId,catalog.Depth,catalog.Tables,catalog.Rows,new UTF8Encoding(false,true).GetString(stream.ToArray()));
                }
                finally{activeTableQuery=false;}
            }
        }
    }
}
