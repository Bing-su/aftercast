using System.Diagnostics;

namespace Aftercast.ActCompat
{
    // Fully materialized in memory; no driver, DSN, database or network operation exists.
    public sealed record OdbcHierarchyPreview(ulong Revision, OdbcPreview[] Tables);
}
namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;
    public partial class FormActMain
    {
        internal OdbcHierarchyPreview ReadOdbcHierarchy(ulong revision, string[][] destinationColumns, string allTerm)
        {
            EnsureTextReportOwner(); ArgumentNullException.ThrowIfNull(destinationColumns);
            if (destinationColumns.Length != 5 || destinationColumns.Any(value => value is null))
                throw new ArgumentException("Five synthetic destination column lists are required");
            var names = destinationColumns.Select(value => value.ToArray()).ToArray();
            foreach (var item in names) ValidateDestinationNames(item);
            allTerm = ColumnText(allTerm);
            lock (CombatGate)
            {
                var capture = hierarchyXml;
                if (capture is null || revision == 0 || revision != capture.Catalog.Revision || capture.Catalog.Depth != HierarchyXmlDepth.AllTables)
                    throw new InvalidOperationException("ODBC hierarchy requires the latest all-tables snapshot");
                var levels = new[] { TableLevel.Encounter, TableLevel.Combatant, TableLevel.Category, TableLevel.Attack, TableLevel.Action };
                var tables = new[] { "encounter_table", "combatant_table", "damagetype_table", "attacktype_table", "swing_table" };
                var snapshots = levels.Select(level => { var catalog = ReadTableColumns(level); return (catalog.Revision, Snapshot:columnSnapshots[level].Snapshot); }).ToArray();
                var outgoing = CombatantData.OutgoingDamageTypeDataObjects.Keys.ToArray();
                var incoming = CombatantData.IncomingDamageTypeDataObjects.Keys.ToArray();
                var outgoingType = outgoing.LastOrDefault() ?? string.Empty;
                var incomingType = incoming.LastOrDefault() ?? string.Empty;
                var clock = Stopwatch.StartNew(); var errors = OwnerErrors.Item1; var bytes = 0;
                void Guard()
                {
                    if (clock.Elapsed > TimeSpan.FromSeconds(4)) throw new TimeoutException("ODBC hierarchy exceeded cooperative four-second budget");
                    if (OwnerErrors.Item1 != errors) throw new InvalidOperationException("ODBC hierarchy callback reported an owner-thread error");
                    if (!outgoing.SequenceEqual(CombatantData.OutgoingDamageTypeDataObjects.Keys, StringComparer.Ordinal) ||
                        !incoming.SequenceEqual(CombatantData.IncomingDamageTypeDataObjects.Keys, StringComparer.Ordinal))
                        throw new InvalidOperationException("ODBC hierarchy category registration changed");
                    for (var i=0;i<levels.Length;i++) RequireTableRevision(levels[i],snapshots[i].Revision);
                }
                void Check() => CheckHierarchy(capture.Root, Guard);
                activeTableQuery = true;
                try
                {
                    Check();
                    var plans = levels.Select((_,i) => OdbcShape(snapshots[i].Snapshot,names[i],tables[i],revision,false).Shape).ToArray();
                    var rows = levels.Select(_ => new List<string[]>()).ToArray();
                    void Add(int index, OdbcPreview plan)
                    {
                        foreach (var row in plan.Parameters)
                        {
                            foreach (var value in row) { bytes = checked(bytes + ColumnUtf8.GetByteCount(value)); if (bytes > MaximumTableTextBytes) throw new InvalidDataException("ODBC hierarchy text exceeds 8 MiB"); }
                            rows[index].Add(row);
                        }
                    }
                    Add(0,BuildOdbcPreview(capture.Root.Capture,names[0],true,false,Check));
                    foreach (var node in HierarchyNodes(capture.Root))
                    {
                        Check(); var level = node.Capture.Scope.Level;
                        if (level == TableLevel.Attack || level == TableLevel.Action)
                        {
                            var category = ((DamageTypeData)node.Capture.Parents[4]).Type;
                            if (level == TableLevel.Attack && category != outgoingType && category != incomingType) continue;
                            if (level == TableLevel.Action && (category != outgoingType || ((AttackType)node.Capture.Parents[6]).Type != allTerm)) continue;
                        }
                        var index = level switch { TableLevel.Combatant=>1, TableLevel.Category=>2, TableLevel.Attack=>3, TableLevel.Action=>4, _=>throw new InvalidDataException("Unexpected hierarchy table") };
                        Add(index,BuildOdbcPreview(node.Capture,names[index],false,false,Check));
                    }
                    Check();
                    for (var i=0;i<plans.Length;i++) plans[i] = plans[i] with { Parameters=rows[i].ToArray() };
                    return new(revision,plans);
                }
                finally { activeTableQuery = false; }
            }
        }
        internal OdbcHierarchyPreview ReadOdbcBatch(ulong revision, string[][] destinationColumns, string allTerm)
        {
            EnsureTextReportOwner(); ArgumentNullException.ThrowIfNull(destinationColumns);
            if (destinationColumns.Length != 5 || destinationColumns.Any(value => value is null))
                throw new ArgumentException("Five synthetic destination column lists are required");
            var names = destinationColumns.Select(value => value.ToArray()).ToArray();
            foreach (var item in names) ValidateDestinationNames(item);
            allTerm = ColumnText(allTerm);
            lock (CombatGate)
            {
                var capture = hierarchyHtml;
                if (capture is null || revision == 0 || revision != capture.Catalog.Revision || capture.Catalog.LastLevel != TableLevel.Action)
                    throw new InvalidOperationException("ODBC batch requires the latest full-depth multi-encounter snapshot");
                var levels = new[] { TableLevel.Encounter, TableLevel.Combatant, TableLevel.Category, TableLevel.Attack, TableLevel.Action };
                var tables = new[] { "encounter_table", "combatant_table", "damagetype_table", "attacktype_table", "swing_table" };
                var snapshots = levels.Select(level => { var catalog = ReadTableColumns(level); return (catalog.Revision, Snapshot:columnSnapshots[level].Snapshot); }).ToArray();
                var outgoing = CombatantData.OutgoingDamageTypeDataObjects.Keys.ToArray();
                var incoming = CombatantData.IncomingDamageTypeDataObjects.Keys.ToArray();
                var outgoingType = outgoing.LastOrDefault() ?? string.Empty;
                var incomingType = incoming.LastOrDefault() ?? string.Empty;
                var clock = Stopwatch.StartNew(); var errors = OwnerErrors.Item1; var bytes = 0;
                void Guard()
                {
                    if (clock.Elapsed > TimeSpan.FromSeconds(4)) throw new TimeoutException("ODBC hierarchy exceeded cooperative four-second budget");
                    if (OwnerErrors.Item1 != errors) throw new InvalidOperationException("ODBC hierarchy callback reported an owner-thread error");
                    if (!outgoing.SequenceEqual(CombatantData.OutgoingDamageTypeDataObjects.Keys, StringComparer.Ordinal) ||
                        !incoming.SequenceEqual(CombatantData.IncomingDamageTypeDataObjects.Keys, StringComparer.Ordinal))
                        throw new InvalidOperationException("ODBC hierarchy category registration changed");
                    for (var i=0;i<levels.Length;i++) RequireTableRevision(levels[i],snapshots[i].Revision);
                }
                void Check() => CheckHierarchies(capture.Roots.Select(root => root.Node), Guard);
                activeTableQuery = true;
                try
                {
                    Check();
                    var plans = levels.Select((_,i) => OdbcShape(snapshots[i].Snapshot,names[i],tables[i],revision,false).Shape).ToArray();
                    var rows = levels.Select(_ => new List<string[]>()).ToArray();
                    void Add(int index, OdbcPreview plan)
                    {
                        foreach (var row in plan.Parameters)
                        {
                            foreach (var value in row) { bytes = checked(bytes + ColumnUtf8.GetByteCount(value)); if (bytes > MaximumTableTextBytes) throw new InvalidDataException("ODBC hierarchy text exceeds 8 MiB"); }
                            rows[index].Add(row);
                        }
                    }
                    foreach (var root in capture.Roots)
                    {
                        Add(0,BuildOdbcPreview(root.Node.Capture,names[0],true,false,Check));
                        foreach (var node in HierarchyNodes(root.Node))
                        {
                        Check(); var level = node.Capture.Scope.Level;
                        if (level == TableLevel.Attack || level == TableLevel.Action)
                        {
                            var category = ((DamageTypeData)node.Capture.Parents[4]).Type;
                            if (level == TableLevel.Attack && category != outgoingType && category != incomingType) continue;
                            if (level == TableLevel.Action && (category != outgoingType || ((AttackType)node.Capture.Parents[6]).Type != allTerm)) continue;
                        }
                        var index = level switch { TableLevel.Combatant=>1, TableLevel.Category=>2, TableLevel.Attack=>3, TableLevel.Action=>4, _=>throw new InvalidDataException("Unexpected hierarchy table") };
                        Add(index,BuildOdbcPreview(node.Capture,names[index],false,false,Check));
                    }
                    }
                    Check();
                    for (var i=0;i<plans.Length;i++) plans[i] = plans[i] with { Parameters=rows[i].ToArray() };
                    return new(revision,plans);
                }
                finally { activeTableQuery = false; }
            }
        }
    }
}
