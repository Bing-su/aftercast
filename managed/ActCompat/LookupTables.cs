using Aftercast.ActCompat;

namespace Advanced_Combat_Tracker;

public static partial class ActGlobals
{
    public static string eDSort = "EncDPS", eDSort2 = "EncDPS";
    public static string mDSort = "Damage", mDSort2 = "Damage", aTSort = "Time", aTSort2 = "Time";
}

public partial class FormActMain
{
    private sealed record LookupGraph(ulong Revision, ulong Encounter, DamageTypeData[] Peers);
    private LookupGraph? lookupGraph;

    // Called under CombatGate at the same admission as the action/log snapshot.
    private void CaptureLookupGraph(ulong revision, EncounterData encounter, LookupRequest request, MasterSwing[] actions)
    {
        var groups = new Dictionary<string, DamageTypeData>(StringComparer.Ordinal);
        foreach (var original in actions)
        {
            var name = request.Outgoing ? original.Victim : original.Attacker;
            if (!groups.TryGetValue(name, out var group))
            {
                if (groups.Count >= 4096) throw new InvalidDataException("Lookup exceeds 4096 peers");
                group = new(request.Outgoing, name, encounter.GetCombatant(name));
                groups.Add(name, group);
            }
            // ACT uses its constructor without Special: None, fresh Tags, no ParentEncounter.
            // Copy mutable Dnum display fields to keep the admitted image independent.
            var amount = new Dnum(original.Damage.Number, original.Damage.RawDamageString) { DamageString2 = original.Damage.DamageString2 };
            var copy = new MasterSwing(original.SwingType, original.Critical, "None", amount, original.Time,
                original.TimeSorter, original.AttackType, original.Attacker, original.DamageType, original.Victim);
            group.AddCombatAction(copy, "All");
            group.AddCombatAction(copy, copy.AttackType);
        }
        lookupGraph = new(revision, Identity(encounter).Id, groups.Values.ToArray());
    }

    internal TablePageData ReadLookupTable(ulong revision, TableQuery query, string? peer = null, string? skill = null)
    {
        if (Environment.CurrentManagedThreadId != ownerThread) throw new InvalidOperationException("Lookup callbacks require owner thread");
        lock (CombatGate)
        {
            ObjectDisposedException.ThrowIf(actionsClosed, this);
            if (activeTableQuery) throw new InvalidOperationException("Reentrant lookup is not supported");
            var graph = lookupGraph;
            if (graph is null || graph.Revision != revision || actionSnapshot?.Revision != revision)
                throw new TableQueryInvalidatedException("Lookup snapshot is stale");
            var level = peer is null ? TableLevel.Category : skill is null ? TableLevel.Attack : TableLevel.Action;
            if (query.MainDescending is not null || query.PublishMainSort || query.Scope.Level != level || query.Scope.EncounterId != graph.Encounter || query.Offset < 0 || query.Limit is < 1 or > 256 ||
                query.Columns is null || query.Columns.Length is < 1 or > 32 || query.Columns.Distinct(StringComparer.Ordinal).Count() != query.Columns.Length)
                throw new ArgumentException("Invalid lookup table request");
            var catalog = ReadTableColumns(level);
            if (catalog.Revision != query.Revision) throw new TableQueryInvalidatedException("Lookup columns changed");
            var columnsSnapshot = columnSnapshots[level].Snapshot;
            var columns = query.Columns.Select(key => SelectTableColumn(columnsSnapshot, key)).ToArray();
            var group = peer is null ? null : graph.Peers.FirstOrDefault(item => item.Type == peer)
                ?? (peer is null ? null : throw new ArgumentException("Unknown lookup peer"));
            object[] rows = peer is null ? graph.Peers.Cast<object>().ToArray() : skill is null ? group!.Items.Values.Cast<object>().ToArray()
                : group!.Items.TryGetValue(skill, out var attack) ? attack.Items.Cast<object>().ToArray() : throw new ArgumentException("Unknown lookup skill");
            if (rows.Length > 32768) throw new InvalidDataException("Lookup table exceeds row limit");
            var primaryKey = query.Sort?.Primary ?? (level == TableLevel.Attack ? ActGlobals.mDSort : ActGlobals.aTSort);
            var secondaryKey = query.Sort?.Secondary ?? (level == TableLevel.Attack ? ActGlobals.mDSort2 : ActGlobals.aTSort2);
            ColumnBinding? Binding(string key) => columnsSnapshot.Bindings.FirstOrDefault(item => item.Metadata.Key == key && item.Sort is not null);
            var primary = level == TableLevel.Category ? null : Binding(primaryKey);
            var secondary = level == TableLevel.Category ? null : Binding(secondaryKey);
            if (query.Sort is not null && (primary is null || query.Sort.Secondary is not null && secondary is null)) throw new ArgumentException("Lookup sort column is unavailable");
            activeTableQuery = true;
            try
            {
                int Compare(int left, int right)
                {
                    var value = primary is null ? 0 : CompareTableRows(level, columnsSnapshot, primary, rows[left], rows[right]);
                    if (value == 0 && secondary is not null) value = CompareTableRows(level, columnsSnapshot, secondary, rows[left], rows[right]);
                    if (value != 0) return value;
                    return (rows[left], rows[right]) switch {
                        (DamageTypeData a, DamageTypeData b) => a.Damage.CompareTo(b.Damage),
                        (AttackType a, AttackType b) => a.Damage.CompareTo(b.Damage),
                        (MasterSwing a, MasterSwing b) => a.TimeSorter != b.TimeSorter ? a.TimeSorter.CompareTo(b.TimeSorter) : a.Time.CompareTo(b.Time),
                        _ => 0
                    };
                }
                var order = TableOrdering.Order(rows.Length, Compare, query.Sort?.Descending ?? true,
                    error => WriteExceptionLog(error, "Lookup sort"), out var sortError);
                var diagnosed = new HashSet<string>(StringComparer.Ordinal);
                var output = new List<TableRowData>(); long bytes = 0;
                foreach (var index in order.Skip(query.Offset).Take(query.Limit))
                {
                    var cells = columns.Select(column => ReadTableCell(level, columnsSnapshot, column, rows[index], diagnosed)).ToArray();
                    bytes += cells.Sum(cell => cell.Text is null ? 0L : ColumnUtf8.GetByteCount(cell.Text));
                    if (bytes > MaximumTableTextBytes) throw new InvalidDataException("Lookup text exceeds 8 MiB");
                    output.Add(new(index, graph.Encounter, peer ?? (rows[index] as DamageTypeData)?.Type, null, skill ?? (rows[index] as AttackType)?.Type, rows[index] is MasterSwing action ? action.TimeSorter : null, cells));
                }
                RequireTableRevision(level, query.Revision);
                if (!ReferenceEquals(lookupGraph, graph) || actionSnapshot?.Revision != revision) throw new TableQueryInvalidatedException("Lookup changed during callbacks");
                ObjectDisposedException.ThrowIf(actionsClosed, this);
                return new(query.Revision, rows.Length, columns.Select(column => column.Metadata).ToArray(), output.ToArray(), sortError);
            }
            finally { activeTableQuery = false; }
        }
    }
}
