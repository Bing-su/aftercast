using System.Drawing;

namespace Aftercast.ActCompat
{
    public sealed record TableScope(TableLevel Level, ulong EncounterId = 0,
        string? Combatant = null, string? Category = null, string? Attack = null, bool ZoneAnchor = false, ulong ZoneId = 0);
    public sealed record TableSort(string Primary, string? Secondary = null, bool Descending = false);
    public sealed record TableQuery(TableScope Scope, ulong Revision, string[] Columns,
        int Offset = 0, int Limit = 16, TableSort? Sort = null, bool? MainDescending = null, bool PublishMainSort = false);
    public sealed record TableCellData(string? Text, int? ForegroundArgb, int? BackgroundArgb, string? Error);
    // Index belongs to this scope snapshot only; it is not a persistent action identity or a source-log link.
    public sealed record TableRowData(int Index, ulong EncounterId, string? Combatant, string? Category,
        string? Attack, int? TimeSorter, TableCellData[] Cells, bool IsAlly = false, bool IsSelf = false);
    public sealed record TablePageData(ulong Revision, int Total, TableColumnData[] Columns,
        TableRowData[] Rows, string? SortError);
    internal sealed class TableQueryInvalidatedException(string message) : InvalidOperationException(message);
}

namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;

    public partial class FormActMain
    {
        private sealed record TableSource(object Value, ulong EncounterId, string? Combatant = null,
            string? Category = null, string? Attack = null, int? TimeSorter = null);
        private bool activeTableQuery;
        private const int MaximumTableRows = 16384, MaximumSelectedColumns = 32;
        private const int MaximumCellBytes = 4096, MaximumTableTextBytes = 8 * 1024 * 1024;

        private static void ValidateTableScope(TableScope scope)
        {
            ArgumentNullException.ThrowIfNull(scope);
            if (!Enum.IsDefined(scope.Level)) throw new ArgumentOutOfRangeException(nameof(scope));
            var depth = (int)scope.Level;
            if ((depth == 0 ? scope.ZoneAnchor != (scope.EncounterId != 0) || scope.ZoneAnchor && scope.ZoneId != 0 : scope.EncounterId == 0 || scope.ZoneAnchor || scope.ZoneId != 0) ||
                (depth >= 2 ? scope.Combatant is null : scope.Combatant is not null) ||
                (depth >= 3 ? scope.Category is null : scope.Category is not null) ||
                (depth >= 4 ? scope.Attack is null : scope.Attack is not null))
                throw new ArgumentException("Table scope does not match its row level");
            foreach (var text in new[] { scope.Combatant, scope.Category, scope.Attack })
                if (text is not null) ColumnText(text);
        }
        private EncounterData? TableEncounter(ulong id)
        {
            var count = 0;
            foreach (var item in QueryEncounters())
            {
                if (++count > MaximumTableRows) throw new InvalidDataException("Table encounter lookup exceeds 16384 rows");
                if (Identity(item.Encounter).Id == id) return item.Encounter;
            }
            return null;
        }
        private IEnumerable<TableSource>? TableSources(TableScope scope)
        {
            if (scope.Level == TableLevel.Encounter)
            {
                if (scope.ZoneId != 0) return QueryZoneId(scope.ZoneId)?.Select(item => new TableSource(item.Encounter, Identity(item.Encounter).Id));
                if (!scope.ZoneAnchor) return QueryEncounters().Select(item => new TableSource(item.Encounter, Identity(item.Encounter).Id));
                return QueryZone(scope.EncounterId)?.Select(item => new TableSource(item.Encounter, Identity(item.Encounter).Id));
            }
            var encounter = TableEncounter(scope.EncounterId);
            if (encounter is null) return null;
            if (scope.Level == TableLevel.Combatant)
                return encounter.Items.Select(pair => new TableSource(pair.Value, scope.EncounterId, ColumnText(pair.Key)));
            var actorKey = scope.Combatant!;
            if (!encounter.Items.TryGetValue(actorKey, out var actor))
            {
                // ACT's encounter.GetCombatant resolves displayed actor names through its uppercase registry key.
                actorKey = actorKey.ToUpper();
                if (!encounter.Items.TryGetValue(actorKey, out actor)) return null;
            }
            if (scope.Level == TableLevel.Category)
                return actor.Items.Select(pair => new TableSource(pair.Value, scope.EncounterId, actorKey, ColumnText(pair.Key)));
            if (!actor.Items.TryGetValue(scope.Category!, out var category)) return null;
            if (scope.Level == TableLevel.Attack)
                return category.Items.Select(pair => new TableSource(pair.Value, scope.EncounterId, actorKey, scope.Category, ColumnText(pair.Key)));
            if (!category.Items.TryGetValue(scope.Attack!, out var attack)) return null;
            return attack.Items.Select(value => new TableSource(value, scope.EncounterId, actorKey,
                scope.Category, scope.Attack, value.TimeSorter));
        }
        private static bool SameBinding(ColumnBinding binding, object definition, string label, bool visible,
            string sqlType, string sqlName, Delegate? cell, Delegate? sql, Delegate? foreground, Delegate? background, Delegate? sort = null) =>
            ReferenceEquals(binding.Definition, definition) && binding.Metadata.Label == label &&
            binding.Metadata.DefaultVisible == visible && binding.Metadata.SqlDataType == sqlType && binding.Metadata.SqlDataName == sqlName &&
            ReferenceEquals(binding.Cell, cell) && ReferenceEquals(binding.Sql, sql) && ReferenceEquals(binding.Foreground, foreground) &&
            ReferenceEquals(binding.Background, background) && ReferenceEquals(binding.Sort, sort);

        private void RequireTableBinding(TableLevel level, ColumnSnapshot snapshot, ColumnBinding binding)
        {
            var key = binding.Metadata.Key;
            // O(1) checks before and after each callback avoid invoking a removed/replaced delegate later in this response.
            var current = level switch
            {
                TableLevel.Encounter => ReferenceEquals(snapshot.Registry, EncounterData.ColumnDefs) && EncounterData.ColumnDefs.Count == snapshot.Bindings.Length &&
                    EncounterData.ColumnDefs.TryGetValue(key, out var value) && value is not null && SameBinding(binding, value, value.Label, value.DefaultVisible,
                        value.SqlDataType, value.SqlDataName, value.GetCellData, value.GetSqlData, value.GetCellForeColor, value.GetCellBackColor),
                TableLevel.Combatant => ReferenceEquals(snapshot.Registry, CombatantData.ColumnDefs) && CombatantData.ColumnDefs.Count == snapshot.Bindings.Length &&
                    CombatantData.ColumnDefs.TryGetValue(key, out var value) && value is not null && SameBinding(binding, value, value.Label, value.DefaultVisible,
                        value.SqlDataType, value.SqlDataName, value.GetCellData, value.GetSqlData, value.GetCellForeColor, value.GetCellBackColor, value.SortComparer),
                TableLevel.Category => ReferenceEquals(snapshot.Registry, DamageTypeData.ColumnDefs) && DamageTypeData.ColumnDefs.Count == snapshot.Bindings.Length &&
                    DamageTypeData.ColumnDefs.TryGetValue(key, out var value) && value is not null && SameBinding(binding, value, value.Label, value.DefaultVisible,
                        value.SqlDataType, value.SqlDataName, value.GetCellData, value.GetSqlData, value.GetCellForeColor, value.GetCellBackColor),
                TableLevel.Attack => ReferenceEquals(snapshot.Registry, AttackType.ColumnDefs) && AttackType.ColumnDefs.Count == snapshot.Bindings.Length &&
                    AttackType.ColumnDefs.TryGetValue(key, out var value) && value is not null && SameBinding(binding, value, value.Label, value.DefaultVisible,
                        value.SqlDataType, value.SqlDataName, value.GetCellData, value.GetSqlData, value.GetCellForeColor, value.GetCellBackColor, value.SortComparer),
                TableLevel.Action => ReferenceEquals(snapshot.Registry, MasterSwing.ColumnDefs) && MasterSwing.ColumnDefs.Count == snapshot.Bindings.Length &&
                    MasterSwing.ColumnDefs.TryGetValue(key, out var value) && value is not null && SameBinding(binding, value, value.Label, value.DefaultVisible,
                        value.SqlDataType, value.SqlDataName, value.GetCellData, value.GetSqlData, value.GetCellForeColor, value.GetCellBackColor, value.SortComparer),
                _ => false
            };
            if (!current)
            {
                columnSnapshots.Remove(level);
                throw new TableQueryInvalidatedException("Column registration changed during table query");
            }
        }
        private static string CellText(ColumnBinding binding, object row) => binding.Cell switch
        {
            EncounterData.StringDataCallback callback => callback((EncounterData)row),
            CombatantData.StringDataCallback callback => callback((CombatantData)row),
            DamageTypeData.StringDataCallback callback => callback((DamageTypeData)row),
            AttackType.StringDataCallback callback => callback((AttackType)row),
            MasterSwing.StringDataCallback callback => callback((MasterSwing)row),
            _ => throw new InvalidDataException("Missing or invalid table cell callback")
        };
        private static int? CellColor(Delegate? callback, object row)
        {
            var color = callback switch
            {
                EncounterData.ColorDataCallback value => value((EncounterData)row),
                CombatantData.ColorDataCallback value => value((CombatantData)row),
                DamageTypeData.ColorDataCallback value => value((DamageTypeData)row),
                AttackType.ColorDataCallback value => value((AttackType)row),
                MasterSwing.ColorDataCallback value => value((MasterSwing)row),
                _ => throw new InvalidDataException("Missing or invalid table color callback")
            };
            return color.IsEmpty || color == Color.Transparent ? null : color.ToArgb();
        }
        private T TableCallback<T>(TableLevel level, ColumnSnapshot snapshot, ColumnBinding binding, Func<T> callback)
        {
            RequireTableBinding(level, snapshot, binding);
            try { return callback(); }
            finally { RequireTableBinding(level, snapshot, binding); }
        }
        private TableCellData ReadTableCell(TableLevel level, ColumnSnapshot snapshot, ColumnBinding binding,
            object row, HashSet<string> diagnosed)
        {
            string? text = null;
            try
            {
                text = TableCallback(level, snapshot, binding, () => CellText(binding, row));
                if (text is null || text.Contains('\0') || ColumnUtf8.GetByteCount(text) > MaximumCellBytes)
                    throw new InvalidDataException("Table cell is null, contains NUL or exceeds 4096 UTF-8 bytes");
                var foreground = TableCallback(level, snapshot, binding, () => CellColor(binding.Foreground, row));
                var background = TableCallback(level, snapshot, binding, () => CellColor(binding.Background, row));
                return new(text, foreground, background, null);
            }
            catch (TableQueryInvalidatedException) { throw; }
            catch (Exception error)
            {
                // Contain one bad plugin cell while preserving valid neighboring cells; diagnose once per column/query.
                if (diagnosed.Add(binding.Metadata.Key)) WriteExceptionLog(error, "Plugin table column: " + binding.Metadata.Key);
                return new(null, null, null, "column-error");
            }
        }
        private int CompareTableRows(TableLevel level, ColumnSnapshot snapshot, ColumnBinding binding, object left, object right) =>
            TableCallback(level, snapshot, binding, () => binding.Sort switch {
                Comparison<CombatantData> callback => callback((CombatantData)left, (CombatantData)right),
                Comparison<AttackType> callback => callback((AttackType)left, (AttackType)right),
                Comparison<MasterSwing> callback => callback((MasterSwing)left, (MasterSwing)right),
                _ => throw new InvalidDataException("Missing or invalid table sort callback")
            });
        private static ColumnBinding SelectTableColumn(ColumnSnapshot snapshot, string key, bool sort = false)
        {
            ArgumentNullException.ThrowIfNull(key);
            var column = snapshot.Bindings.FirstOrDefault(binding => binding.Metadata.Key == key)
                ?? throw new ArgumentException("Unregistered table column: " + ColumnText(key));
            if (sort && column.Sort is null) throw new ArgumentException("Table column has no sort callback: " + key);
            return column;
        }
        internal TablePageData? ReadTable(TableQuery query)
        {
            ArgumentNullException.ThrowIfNull(query);
            ValidateTableScope(query.Scope);
            if (query.Offset < 0 || query.Limit is < 1 or > 256 || query.Revision == 0 ||
                query.Columns is null || query.Columns.Length is < 1 or > MaximumSelectedColumns)
                throw new ArgumentException("Invalid table page, revision or selected-column count");
            var keys = query.Columns.ToArray();
            if (keys.Distinct(StringComparer.Ordinal).Count() != keys.Length) throw new ArgumentException("Duplicate selected columns");
            if (Environment.CurrentManagedThreadId != ownerThread) throw new InvalidOperationException("Tables must be read on the host owner thread");
            ObjectDisposedException.ThrowIf(actionsClosed, this);
            lock (CombatGate)
            {
                if (activeTableQuery) throw new InvalidOperationException("Reentrant table queries are not supported");
                var catalog = ReadTableColumns(query.Scope.Level);
                if (catalog.Revision != query.Revision) throw new TableQueryInvalidatedException("Column catalog revision mismatch");
                var snapshot = columnSnapshots[query.Scope.Level].Snapshot;
                var columns = keys.Select(key => SelectTableColumn(snapshot, key)).ToArray();
                if (query.PublishMainSort && (query.MainDescending is null || query.Sort is null))
                    throw new ArgumentException("Publishing requires an explicit main sort");
                if (query.MainDescending is not null && query.Scope.Level is not (TableLevel.Combatant or TableLevel.Attack or TableLevel.Action))
                    throw new ArgumentException("Main sorting is unavailable at this table level");
                string? defaultPrimary = query.MainDescending is null ? null : query.Scope.Level switch {
                    TableLevel.Combatant => ActGlobals.eDSort, TableLevel.Attack => ActGlobals.mDSort,
                    TableLevel.Action => ActGlobals.aTSort, _ => null };
                string? defaultSecondary = query.MainDescending is null ? null : query.Scope.Level switch {
                    TableLevel.Combatant => ActGlobals.eDSort2, TableLevel.Attack => ActGlobals.mDSort2,
                    TableLevel.Action => ActGlobals.aTSort2, _ => null };
                ColumnBinding? DefaultBinding(string? key) => key is null ? null : snapshot.Bindings.FirstOrDefault(value => value.Metadata.Key == key);
                var primary = query.Sort is null ? DefaultBinding(defaultPrimary) : SelectTableColumn(snapshot, query.Sort.Primary, true);
                var secondary = query.Sort is null ? DefaultBinding(defaultSecondary) : query.Sort.Secondary is null ? null : SelectTableColumn(snapshot, query.Sort.Secondary, true);
                var source = TableSources(query.Scope);
                if (source is null) return null;
                var rows = source.Take(MaximumTableRows + 1).ToArray();
                if (rows.Any(row => row.Value is null)) throw new InvalidDataException("Table graph contains a null row");
                if (rows.Length > MaximumTableRows) throw new InvalidDataException("Table scope exceeds 16384 rows; select a narrower scope");
                if (query.PublishMainSort) PublishMainSort(query.Scope.Level, query.Sort!);
                activeTableQuery = true;
                try { return BuildTable(query, snapshot, columns, primary, secondary, rows); }
                finally { activeTableQuery = false; }
            }
        }
        private static void PublishMainSort(TableLevel level, TableSort sort)
        {
            static void Update(ref string primary, ref string secondary, TableSort requested)
            {
                if (requested.Secondary is not null) secondary = requested.Secondary;
                else if (primary != requested.Primary) secondary = primary;
                primary = requested.Primary;
            }
            switch (level)
            {
                case TableLevel.Combatant: Update(ref ActGlobals.eDSort, ref ActGlobals.eDSort2, sort); break;
                case TableLevel.Attack: Update(ref ActGlobals.mDSort, ref ActGlobals.mDSort2, sort); break;
                case TableLevel.Action: Update(ref ActGlobals.aTSort, ref ActGlobals.aTSort2, sort); break;
                default: throw new ArgumentException("Main sort level is invalid");
            }
        }
        private TablePageData BuildTable(TableQuery query, ColumnSnapshot snapshot, ColumnBinding[] columns,
            ColumnBinding? primary, ColumnBinding? secondary, TableSource[] rows)
        {
            var level = query.Scope.Level;
            int Compare(int left, int right)
            {
                var value = primary is null ? 0 : CompareTableRows(level, snapshot, primary, rows[left].Value, rows[right].Value);
                if (value == 0 && secondary is not null) value = CompareTableRows(level, snapshot, secondary, rows[left].Value, rows[right].Value);
                if (value != 0 || query.MainDescending is null) return value;
                return (rows[left].Value, rows[right].Value) switch {
                    (CombatantData a, CombatantData b) => a.Damage.CompareTo(b.Damage),
                    (AttackType a, AttackType b) => a.Damage.CompareTo(b.Damage),
                    (MasterSwing a, MasterSwing b) => a.TimeSorter != b.TimeSorter ? a.TimeSorter.CompareTo(b.TimeSorter) : a.Time.CompareTo(b.Time),
                    _ => 0 };

            }
            string? sortError = null;
            var order = primary is null && query.MainDescending is null ? Enumerable.Range(0, rows.Length).ToArray()
                : TableOrdering.Order(rows.Length, Compare, query.MainDescending ?? query.Sort!.Descending,
                    error => WriteExceptionLog(error, "Plugin table sort"), out sortError, reverseTies: query.MainDescending is not null);
            RequireTableRevision(level, query.Revision);
            var diagnosed = new HashSet<string>(StringComparer.Ordinal);
            var character = ActGlobals.charName;
            var parent = level == TableLevel.Combatant ? rows.FirstOrDefault()?.Value as CombatantData : null;
            var encounter = parent?.Parent;
            var allies = encounter?.GetAllies().ToArray();
            var result = new List<TableRowData>();
            long bytes = 0;
            foreach (var index in order.Skip(query.Offset).Take(query.Limit))
            {
                var row = rows[index];
                var cells = columns.Select(column => ReadTableCell(level, snapshot, column, row.Value, diagnosed)).ToArray();
                bytes += cells.Sum(cell => cell.Text is null ? 0L : ColumnUtf8.GetByteCount(cell.Text));
                if (bytes > MaximumTableTextBytes) throw new InvalidDataException("Table response text exceeds 8 MiB");
                var actor = level == TableLevel.Combatant ? row.Value as CombatantData : null;
                result.Add(new(index, row.EncounterId, row.Combatant, row.Category, row.Attack, row.TimeSorter, cells,
                    actor is not null && allies is not null && allies.Contains(actor), actor is not null && actor.Name == character));
            }
            RequireTableRevision(level, query.Revision);
            if (level == TableLevel.Combatant && result.Any(item => rows[item.Index].Value is CombatantData actor && item.IsSelf != (actor.Name == character)))
                throw new TableQueryInvalidatedException("Combatant personal classification changed during callbacks");
            if (level == TableLevel.Combatant && (character != ActGlobals.charName || (encounter is not null && allies is not null && !allies.SequenceEqual(encounter.GetAllies(), ReferenceEqualityComparer.Instance))))
                throw new TableQueryInvalidatedException("Combatant color classification changed during callbacks");
            var current = TableSources(query.Scope)?.Take(MaximumTableRows + 1).ToArray();
            if (current is null || rows.Length != current.Length || !rows.Zip(current).All(pair =>
                ReferenceEquals(pair.First.Value, pair.Second.Value) && pair.First.EncounterId == pair.Second.EncounterId &&
                pair.First.Combatant == pair.Second.Combatant && pair.First.Category == pair.Second.Category &&
                pair.First.Attack == pair.Second.Attack && pair.First.TimeSorter == pair.Second.TimeSorter))
                throw new TableQueryInvalidatedException("Table scope membership changed during callbacks");
            ObjectDisposedException.ThrowIf(actionsClosed, this);
            return new(query.Revision, rows.Length, columns.Select(column => column.Metadata).ToArray(), result.ToArray(), sortError);
        }
        private void RequireTableRevision(TableLevel level, ulong expected)
        {
            if (ReadTableColumns(level).Revision != expected) throw new TableQueryInvalidatedException("Column catalog changed during table query");
        }
    }
}
