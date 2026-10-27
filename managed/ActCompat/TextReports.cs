using System.Diagnostics;
using System.Text;
using System.Text.RegularExpressions;
using Advanced_Combat_Tracker;

namespace Aftercast.ActCompat
{
    // Explicit unaligned text only; HTML/table XML and native font/RTF output are separate formats.
    public sealed record TextReportOptions(string PlayerFormat, string Sorting, bool ShowOnlyAllies,
        bool ShowAlliedInfo, string AlliesFormat)
    {
        public void Validate()
        {
            foreach (var template in new[] { PlayerFormat, AlliesFormat })
            {
                ArgumentNullException.ThrowIfNull(template);
                if (new UTF8Encoding(false,true).GetByteCount(template)>16384) throw new ArgumentException("Report template exceeds 16 KiB UTF-8");
            }
            ArgumentNullException.ThrowIfNull(Sorting);
            if (Sorting.Any(char.IsControl) || new UTF8Encoding(false,true).GetByteCount(Sorting)>256)
                throw new ArgumentException("Report sort key must be control-free and at most 256 UTF-8 bytes");
        }
        internal TextExportFormatOptions Format => new(PlayerFormat,Sorting,ShowOnlyAllies,ShowAlliedInfo,AlliesFormat);
    }
    public sealed record TextReportCatalog(ulong Revision,string[] PlayerVariables,string[] AlliedVariables,string[] SortColumns);
    public sealed record TextReport(ulong EncounterId,ulong Revision,string Text);
}
namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;
    public partial class FormActMain
    {
        private TextReportSnapshot? textReportSnapshot;
        private ulong textReportRevision;
        private sealed class TextReportSnapshot(FormActMain root)
        {
            internal readonly object PlayersRegistry=CombatantData.ExportVariables, AlliedRegistry=EncounterData.ExportVariables;
            internal readonly Dictionary<string,(CombatantData.TextExportFormatter Definition,CombatantData.ExportStringDataCallback Callback)> Players=
                CombatantData.ExportVariables.Take(1025).ToDictionary(pair=>pair.Key,pair=>(pair.Value,pair.Value.GetExportString),StringComparer.Ordinal);
            internal readonly Dictionary<string,(EncounterData.TextExportFormatter Definition,EncounterData.ExportStringDataCallback Callback)> Allied=
                EncounterData.ExportVariables.Take(1025).ToDictionary(pair=>pair.Key,pair=>(pair.Value,pair.Value.GetExportString),StringComparer.Ordinal);
            internal readonly Regex Parser=root.TextExportFormatterRegex;
            internal readonly string Open=root.TextExportFormatterCap1,Close=root.TextExportFormatterCap2;
            internal readonly TableColumnCatalog Columns=root.ReadTableColumns(TableLevel.Combatant);
            internal bool Same(FormActMain root,TextReportSnapshot other) =>
                ReferenceEquals(PlayersRegistry,other.PlayersRegistry)&&ReferenceEquals(AlliedRegistry,other.AlliedRegistry)&&
                ReferenceEquals(Parser,other.Parser)&&Open==other.Open&&Close==other.Close&&Columns.Revision==other.Columns.Revision&&
                Players.Count==other.Players.Count&&Allied.Count==other.Allied.Count&&
                Players.All(pair=>other.Players.TryGetValue(pair.Key,out var value)&&ReferenceEquals(pair.Value.Definition,value.Definition)&&ReferenceEquals(pair.Value.Callback,value.Callback))&&
                Allied.All(pair=>other.Allied.TryGetValue(pair.Key,out var value)&&ReferenceEquals(pair.Value.Definition,value.Definition)&&ReferenceEquals(pair.Value.Callback,value.Callback));
        }
        private void EnsureTextReportOwner()
        {
            if(Environment.CurrentManagedThreadId!=ownerThread) throw new InvalidOperationException("Reports require the host owner thread");
            ObjectDisposedException.ThrowIf(actionsClosed,this);
            if(activeTableQuery) throw new InvalidOperationException("Reentrant reports/table queries are not supported");
        }
        private TextReportSnapshot CaptureTextReport()
        {
            if(CombatantData.ExportVariables is null||EncounterData.ExportVariables is null ||
                CombatantData.ExportVariables.Count>1024||EncounterData.ExportVariables.Count>1024)
                throw new InvalidDataException("Report variable registries must contain at most 1024 entries each");
            foreach(var key in CombatantData.ExportVariables.Keys.Concat(EncounterData.ExportVariables.Keys))
                if(key is null||new UTF8Encoding(false,true).GetByteCount(key)>256) throw new InvalidDataException("Report variable key exceeds 256 UTF-8 bytes");
            if(CombatantData.ExportVariables.Values.Any(value=>value?.GetExportString is null)||EncounterData.ExportVariables.Values.Any(value=>value?.GetExportString is null))
                throw new InvalidDataException("Null report formatter definition/callback");
            if(TextExportFormatterRegex is null||TextExportFormatterCap1 is null||TextExportFormatterCap2 is null||
                new UTF8Encoding(false,true).GetByteCount(TextExportFormatterCap1)+new UTF8Encoding(false,true).GetByteCount(TextExportFormatterCap2)>1024)
                throw new InvalidDataException("Invalid report parser/caps");
            if(new UTF8Encoding(false,true).GetByteCount(TextExportFormatterRegex.ToString())>16384)
                throw new InvalidDataException("Report parser pattern exceeds 16 KiB UTF-8");
            var snapshot=new TextReportSnapshot(this);
            if(snapshot.Players.Count>1024||snapshot.Allied.Count>1024)throw new InvalidDataException("Report variable registry grew beyond its bound");
            return snapshot;
        }
        internal TextReportCatalog ReadTextReportCatalog()
        {
            EnsureTextReportOwner();lock(CombatGate) {
                var captured=CaptureTextReport();
                if(textReportSnapshot is null||!textReportSnapshot.Same(this,captured)) {textReportSnapshot=captured;textReportRevision=checked(textReportRevision+1);}
                return new(textReportRevision,captured.Players.Keys.ToArray(),captured.Allied.Keys.ToArray(),
                    captured.Columns.Columns.Where(column=>column.CanSort).Select(column=>column.Key).ToArray());
            }
        }
        internal TextReport? ReadTextReport(ulong encounterId,ulong revision,TextReportOptions options)
        {
            ArgumentNullException.ThrowIfNull(options);options.Validate();EnsureTextReportOwner();
            if(encounterId==0||revision==0) throw new ArgumentException("Report requires an encounter and catalog revision");
            lock(CombatGate) {
                var catalog=ReadTextReportCatalog();
                if(catalog.Revision!=revision) throw new InvalidOperationException("Report catalog revision mismatch");
                var scope=FindTextReportScope(encounterId);if(scope is null)return null;
                var encounter=scope.Encounter;
                var execution=new TextReportExecution(this,textReportSnapshot!,options.Sorting,scope);
                activeTableQuery=true;
                try {
                    var text=BuildTextExport(encounter,options.Format,execution);execution.Check();
                    if(!textReportSnapshot!.Same(this,CaptureTextReport()))throw new InvalidOperationException("Report registry changed during rendering");
                    return new(encounterId,revision,text);
                }
                finally {activeTableQuery=false;}
            }
        }
        private sealed class TextReportExecution : IComparer<CombatantData>
        {
            private readonly FormActMain root;
            private readonly TextReportSnapshot snapshot;
            private readonly string sorting;
            private readonly TextReportScope scope;
            private readonly long ownerErrors;
            private readonly ColumnSnapshot columns;
            private readonly ColumnBinding? sort;
            private readonly Stopwatch timer=Stopwatch.StartNew();
            private int bytes;
            internal Regex Parser {get;}
            internal TextReportExecution(FormActMain root,TextReportSnapshot snapshot,string sorting,TextReportScope scope)
            {
                this.root=root;this.snapshot=snapshot;this.sorting=sorting;this.scope=scope;ownerErrors=root.OwnerErrors.Item1;
                columns=root.columnSnapshots[TableLevel.Combatant].Snapshot;
                sort=columns.Bindings.FirstOrDefault(binding=>binding.Metadata.Key==sorting);
                Parser=new Regex(snapshot.Parser.ToString(),snapshot.Parser.Options,TimeSpan.FromMilliseconds(100));
            }
            internal void BindAllies(List<CombatantData> allies,bool allActors)=>scope.BindAllies(allies,allActors);
            internal void ValidateActors(int count) {if(count>16384)throw new InvalidDataException("Report scope exceeds 16384 combatants");}
            internal void Check()
            {
                if(timer.Elapsed>TimeSpan.FromSeconds(2)) throw new TimeoutException("Report exceeded the cooperative two-second budget");
                scope.Check();
                if(root.OwnerErrors.Item1!=ownerErrors)throw new InvalidOperationException("Report callback reported an owner-thread exception");
                if(!ReferenceEquals(snapshot.PlayersRegistry,CombatantData.ExportVariables)||snapshot.Players.Count!=CombatantData.ExportVariables.Count||
                    !ReferenceEquals(snapshot.AlliedRegistry,EncounterData.ExportVariables)||snapshot.Allied.Count!=EncounterData.ExportVariables.Count||
                    !ReferenceEquals(snapshot.Parser,root.TextExportFormatterRegex)||snapshot.Open!=root.TextExportFormatterCap1||snapshot.Close!=root.TextExportFormatterCap2)
                    throw new InvalidOperationException("Report registry changed during rendering");
            }
            internal void Append(string? value)
            {
                Check();if(value is null)return;
                var size=new UTF8Encoding(false,true).GetByteCount(value);
                if(size>1024*1024-bytes)throw new InvalidDataException("Report output exceeds 1 MiB UTF-8");bytes+=size;
            }
            internal string Player(string key,CombatantData actor,string extra)
            {
                Check();if(!snapshot.Players.TryGetValue(key,out var entry))return snapshot.Open+key+snapshot.Close;
                void Validate() {Check();if(!CombatantData.ExportVariables.TryGetValue(key,out var value)||!ReferenceEquals(entry.Definition,value)||!ReferenceEquals(entry.Callback,value.GetExportString))throw new InvalidOperationException("Report formatter changed during callback");}
                return Invoke(()=>entry.Callback(actor,extra),Validate);
            }
            internal string Allied(string key,EncounterData encounter,List<CombatantData> allies,string extra)
            {
                Check();if(!snapshot.Allied.TryGetValue(key,out var entry))return snapshot.Open+key+snapshot.Close;
                void Validate() {Check();if(!EncounterData.ExportVariables.TryGetValue(key,out var value)||!ReferenceEquals(entry.Definition,value)||!ReferenceEquals(entry.Callback,value.GetExportString))throw new InvalidOperationException("Report formatter changed during callback");}
                return Invoke(()=>entry.Callback(encounter,allies,extra),Validate);
            }
            private int CompareOnce(CombatantData left,CombatantData right)
            {
                Check();if(sort is null) {if(CombatantData.ColumnDefs.ContainsKey(sorting))throw new InvalidOperationException("Report sort appeared during rendering");return 0;}
                void Validate() {Check();root.RequireTableBinding(TableLevel.Combatant,columns,sort);}
                return Invoke(()=>((Comparison<CombatantData>?)sort.Sort??throw new InvalidDataException("Report column has no sort callback"))(left,right),Validate);
            }
            private static T Invoke<T>(Func<T> callback,Action validate)
            {
                validate();Exception? primary=null;
                try{return callback();}
                catch(Exception error){primary=error;throw;}
                finally {
                    try{validate();}
                    catch(Exception after) when(primary is not null) {throw new AggregateException("Report callback and validation both failed",primary,after);}
                }
            }
            public int Compare(CombatantData? left,CombatantData? right)
            {
                var result=CompareOnce(left!,right!);if(result==0)result=CompareOnce(left!,right!);
                return result!=0?result:left!.Damage.CompareTo(right!.Damage);
            }
        }
    }
}
