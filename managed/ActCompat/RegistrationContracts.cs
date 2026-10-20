using System.Drawing;

namespace Advanced_Combat_Tracker;

// These registries retain plugin definitions and callbacks; they do not manufacture combat state.
public partial class CombatantData
{
    private CombatantData() { }
    public static Dictionary<string, DamageTypeDef> IncomingDamageTypeDataObjects = new();
    public static Dictionary<string, DamageTypeDef> OutgoingDamageTypeDataObjects = new();
    public static SortedDictionary<int, List<string>> SwingTypeToDamageTypeDataLinksIncoming = new();
    public static SortedDictionary<int, List<string>> SwingTypeToDamageTypeDataLinksOutgoing = new();
    public static List<int> DamageSwingTypes = new();
    public static List<int> HealingSwingTypes = new();
    public static Dictionary<string, TextExportFormatter> ExportVariables = new();
    public static Dictionary<string, ColumnDef> ColumnDefs = new();
    public string GetColumnByName(string name) => ColumnDefs.TryGetValue(name, out var column) ? column.GetCellData(this) : string.Empty;

    public class DamageTypeDef(string label, int allyValue, Color typeColor)
    {
        public string Label { get; } = label;
        public int AllyValue { get; } = allyValue;
        public Color TypeColor { get; } = typeColor;
    }

    public delegate string ExportStringDataCallback(CombatantData data, string extra);
    public class TextExportFormatter(string name, string label, string description, ExportStringDataCallback callback)
    {
        public string Name { get; } = name;
        public string Label { get; } = label;
        public string Description { get; } = description;
        public ExportStringDataCallback GetExportString = callback;
    }

    public delegate string StringDataCallback(CombatantData data);
    public delegate Color ColorDataCallback(CombatantData data);
    public class ColumnDef(string label, bool defaultVisible, string sqlDataType, string sqlDataName,
        StringDataCallback cell, StringDataCallback sql, Comparison<CombatantData> sort)
    {
        public string Label { get; } = label;
        public bool DefaultVisible { get; } = defaultVisible;
        public string SqlDataType { get; } = sqlDataType;
        public string SqlDataName { get; } = sqlDataName;
        public StringDataCallback GetCellData = cell;
        public StringDataCallback GetSqlData = sql;
        public Comparison<CombatantData> SortComparer = sort;
        public ColorDataCallback GetCellForeColor = static _ => Color.Transparent;
        public ColorDataCallback GetCellBackColor = static _ => Color.Transparent;
    }
}

public partial class EncounterData
{
    private EncounterData() { }
    public static Dictionary<string, TextExportFormatter> ExportVariables = new();
    public static Dictionary<string, ColumnDef> ColumnDefs = new();
    public string GetColumnByName(string name) => ColumnDefs.TryGetValue(name, out var column) ? column.GetCellData(this) : string.Empty;
    public delegate string ExportStringDataCallback(EncounterData data, List<CombatantData> allies, string extra);
    public class TextExportFormatter(string name, string label, string description, ExportStringDataCallback callback)
    {
        public string Name { get; } = name;
        public string Label { get; } = label;
        public string Description { get; } = description;
        public ExportStringDataCallback GetExportString = callback;
    }

    public delegate string StringDataCallback(EncounterData data);
    public delegate Color ColorDataCallback(EncounterData data);
    public class ColumnDef(string label, bool defaultVisible, string sqlDataType, string sqlDataName,
        StringDataCallback cell, StringDataCallback sql)
    {
        public string Label { get; } = label;
        public bool DefaultVisible { get; } = defaultVisible;
        public string SqlDataType { get; } = sqlDataType;
        public string SqlDataName { get; } = sqlDataName;
        public StringDataCallback GetCellData = cell;
        public StringDataCallback GetSqlData = sql;
        public ColorDataCallback GetCellForeColor = static _ => Color.Transparent;
        public ColorDataCallback GetCellBackColor = static _ => Color.Transparent;
    }
}

public partial class MasterSwing
{
    private MasterSwing() { }
    public static Dictionary<string, ColumnDef> ColumnDefs = new();
    public string GetColumnByName(string name) => ColumnDefs.TryGetValue(name, out var column) ? column.GetCellData(this) : string.Empty;
    public delegate string StringDataCallback(MasterSwing data);
    public delegate Color ColorDataCallback(MasterSwing data);
    public class ColumnDef(string label, bool defaultVisible, string sqlDataType, string sqlDataName,
        StringDataCallback cell, StringDataCallback sql, Comparison<MasterSwing> sort)
    {
        public string Label { get; } = label;
        public bool DefaultVisible { get; } = defaultVisible;
        public string SqlDataType { get; } = sqlDataType;
        public string SqlDataName { get; } = sqlDataName;
        public StringDataCallback GetCellData = cell;
        public StringDataCallback GetSqlData = sql;
        public Comparison<MasterSwing> SortComparer = sort;
        public ColorDataCallback GetCellForeColor = static _ => Color.Transparent;
        public ColorDataCallback GetCellBackColor = static _ => Color.Transparent;
    }
}

public partial class AttackType
{
    private AttackType() { }
    public static Dictionary<string, ColumnDef> ColumnDefs = new();
    public string GetColumnByName(string name) => ColumnDefs.TryGetValue(name, out var column) ? column.GetCellData(this) : string.Empty;
    public delegate string StringDataCallback(AttackType data);
    public delegate Color ColorDataCallback(AttackType data);
    public class ColumnDef(string label, bool defaultVisible, string sqlDataType, string sqlDataName,
        StringDataCallback cell, StringDataCallback sql, Comparison<AttackType> sort)
    {
        public string Label { get; } = label;
        public bool DefaultVisible { get; } = defaultVisible;
        public string SqlDataType { get; } = sqlDataType;
        public string SqlDataName { get; } = sqlDataName;
        public StringDataCallback GetCellData = cell;
        public StringDataCallback GetSqlData = sql;
        public Comparison<AttackType> SortComparer = sort;
        public ColorDataCallback GetCellForeColor = static _ => Color.Transparent;
        public ColorDataCallback GetCellBackColor = static _ => Color.Transparent;
    }
}

public partial class DamageTypeData
{
    private DamageTypeData() { }
    public static Dictionary<string, ColumnDef> ColumnDefs = new();
    public string GetColumnByName(string name) => ColumnDefs.TryGetValue(name, out var column) ? column.GetCellData(this) : string.Empty;
    public delegate string StringDataCallback(DamageTypeData data);
    public delegate Color ColorDataCallback(DamageTypeData data);
    public class ColumnDef(string label, bool defaultVisible, string sqlDataType, string sqlDataName,
        StringDataCallback cell, StringDataCallback sql)
    {
        public string Label { get; } = label;
        public bool DefaultVisible { get; } = defaultVisible;
        public string SqlDataType { get; } = sqlDataType;
        public string SqlDataName { get; } = sqlDataName;
        public StringDataCallback GetCellData = cell;
        public StringDataCallback GetSqlData = sql;
        public ColorDataCallback GetCellForeColor = static _ => Color.Transparent;
        public ColorDataCallback GetCellBackColor = static _ => Color.Transparent;
    }
}
