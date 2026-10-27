using System.Text;

namespace Aftercast.ActCompat
{
    // These are the row types, e.g. selecting an encounter displays Combatant columns.
    public enum TableLevel { Encounter, Combatant, Category, Attack, Action }
    public sealed record TableColumnData(string Key, string Label, bool DefaultVisible,
        string SqlDataType, string SqlDataName, bool CanRead, bool CanSort);
    public sealed record TableCategoryData(string Key, string Label, bool Outgoing, int AllyValue, int ColorArgb);
    public sealed record TableColumnCatalog(TableLevel Level, ulong Revision,
        TableColumnData[] Columns, TableCategoryData[] Categories);
}

namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;

    public partial class FormActMain
    {
        private sealed record ColumnBinding(TableColumnData Metadata, object Definition,
            Delegate? Cell, Delegate? Sql, Delegate? Foreground, Delegate? Background, Delegate? Sort);
        private sealed record ColumnSnapshot(object Registry, ColumnBinding[] Bindings, TableCategoryData[] Categories);
        private readonly Dictionary<TableLevel, (ulong Revision, ColumnSnapshot Snapshot)> columnSnapshots = new();
        private ulong nextColumnRevision;
        private static readonly UTF8Encoding ColumnUtf8 = new(false, true);
        private const int MaximumColumns = 256, MaximumCategories = 256, MaximumColumnTextBytes = 1024;

        private static string ColumnText(string? value)
        {
            if (value is null || value.Contains('\0') || ColumnUtf8.GetByteCount(value) > MaximumColumnTextBytes)
                throw new InvalidDataException("Column metadata text is null, contains NUL, has invalid UTF-8 or exceeds 1024 bytes");
            return value;
        }
        private static ColumnBinding Bind(string key, string label, bool visible, string sqlType, string sqlName,
            object definition, Delegate? cell, Delegate? sql, Delegate? foreground, Delegate? background, Delegate? sort = null)
        {
            if (string.IsNullOrEmpty(key)) throw new InvalidDataException("Column keys must not be empty");
            return new(new(ColumnText(key), ColumnText(label), visible, ColumnText(sqlType), ColumnText(sqlName),
                cell is not null, sort is not null), definition, cell, sql, foreground, background, sort);
        }
        private static ColumnBinding[] CaptureColumns(TableLevel level) => level switch
        {
            TableLevel.Encounter => EncounterData.ColumnDefs.Select(pair => {
                var value = pair.Value ?? throw new InvalidDataException("Null encounter column");
                return Bind(pair.Key, value.Label, value.DefaultVisible, value.SqlDataType, value.SqlDataName,
                    value, value.GetCellData, value.GetSqlData, value.GetCellForeColor, value.GetCellBackColor);
            }).Take(MaximumColumns + 1).ToArray(),
            TableLevel.Combatant => CombatantData.ColumnDefs.Select(pair => {
                var value = pair.Value ?? throw new InvalidDataException("Null combatant column");
                return Bind(pair.Key, value.Label, value.DefaultVisible, value.SqlDataType, value.SqlDataName,
                    value, value.GetCellData, value.GetSqlData, value.GetCellForeColor, value.GetCellBackColor, value.SortComparer);
            }).Take(MaximumColumns + 1).ToArray(),
            TableLevel.Category => DamageTypeData.ColumnDefs.Select(pair => {
                var value = pair.Value ?? throw new InvalidDataException("Null category column");
                return Bind(pair.Key, value.Label, value.DefaultVisible, value.SqlDataType, value.SqlDataName,
                    value, value.GetCellData, value.GetSqlData, value.GetCellForeColor, value.GetCellBackColor);
            }).Take(MaximumColumns + 1).ToArray(),
            TableLevel.Attack => AttackType.ColumnDefs.Select(pair => {
                var value = pair.Value ?? throw new InvalidDataException("Null attack column");
                return Bind(pair.Key, value.Label, value.DefaultVisible, value.SqlDataType, value.SqlDataName,
                    value, value.GetCellData, value.GetSqlData, value.GetCellForeColor, value.GetCellBackColor, value.SortComparer);
            }).Take(MaximumColumns + 1).ToArray(),
            TableLevel.Action => MasterSwing.ColumnDefs.Select(pair => {
                var value = pair.Value ?? throw new InvalidDataException("Null action column");
                return Bind(pair.Key, value.Label, value.DefaultVisible, value.SqlDataType, value.SqlDataName,
                    value, value.GetCellData, value.GetSqlData, value.GetCellForeColor, value.GetCellBackColor, value.SortComparer);
            }).Take(MaximumColumns + 1).ToArray(),
            _ => throw new ArgumentOutOfRangeException(nameof(level))
        };
        private static object ColumnRegistry(TableLevel level) => level switch
        {
            TableLevel.Encounter => EncounterData.ColumnDefs,
            TableLevel.Combatant => CombatantData.ColumnDefs,
            TableLevel.Category => DamageTypeData.ColumnDefs,
            TableLevel.Attack => AttackType.ColumnDefs,
            TableLevel.Action => MasterSwing.ColumnDefs,
            _ => throw new ArgumentOutOfRangeException(nameof(level))
        };
        private static TableCategoryData[] CaptureCategories()
        {
            TableCategoryData[] Read(Dictionary<string, CombatantData.DamageTypeDef> registry, bool outgoing)
            {
                if (registry is null) throw new InvalidDataException("Null category registry");
                if (registry.Count > MaximumCategories) throw new InvalidDataException("Category count exceeds 256 per direction");
                return registry.Select(pair => {
                    var value = pair.Value ?? throw new InvalidDataException("Null category definition");
                    if (string.IsNullOrEmpty(pair.Key)) throw new InvalidDataException("Category keys must not be empty");
                    return new TableCategoryData(ColumnText(pair.Key), ColumnText(value.Label), outgoing, value.AllyValue, value.TypeColor.ToArgb());
                }).ToArray();
            }
            return Read(CombatantData.OutgoingDamageTypeDataObjects, true)
                .Concat(Read(CombatantData.IncomingDamageTypeDataObjects, false)).ToArray();
        }
        private static bool SameColumns(ColumnSnapshot left, ColumnSnapshot right)
        {
            if (!ReferenceEquals(left.Registry, right.Registry) || left.Bindings.Length != right.Bindings.Length ||
                !left.Categories.SequenceEqual(right.Categories)) return false;
            return left.Bindings.Zip(right.Bindings).All(pair =>
                pair.First.Metadata == pair.Second.Metadata && ReferenceEquals(pair.First.Definition, pair.Second.Definition) &&
                ReferenceEquals(pair.First.Cell, pair.Second.Cell) && ReferenceEquals(pair.First.Sql, pair.Second.Sql) &&
                ReferenceEquals(pair.First.Foreground, pair.Second.Foreground) && ReferenceEquals(pair.First.Background, pair.Second.Background) &&
                ReferenceEquals(pair.First.Sort, pair.Second.Sort));
        }
        internal TableColumnCatalog ReadTableColumns(TableLevel level)
        {
            if (Environment.CurrentManagedThreadId != ownerThread)
                throw new InvalidOperationException("Column catalogs must be read on the host owner thread");
            ObjectDisposedException.ThrowIf(actionsClosed, this);
            lock (CombatGate)
            {
                var registry = ColumnRegistry(level) ?? throw new InvalidDataException("Null column registry");
                var bindings = CaptureColumns(level);
                if (bindings.Length > MaximumColumns) throw new InvalidDataException("Column count exceeds 256 per level");
                var snapshot = new ColumnSnapshot(registry, bindings, CaptureCategories());
                if (!columnSnapshots.TryGetValue(level, out var current) || !SameColumns(current.Snapshot, snapshot))
                {
                    current = (checked(++nextColumnRevision), snapshot);
                    columnSnapshots[level] = current;
                }
                // Arrays are caller-owned; mutable public ACT registries never escape as response collections.
                return new(level, current.Revision, bindings.Select(binding => binding.Metadata).ToArray(), snapshot.Categories.ToArray());
            }
        }
    }
}
