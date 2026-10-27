using System.Drawing;
using System.Windows.Forms;
using Aftercast.ActCompat;
using Advanced_Combat_Tracker;

internal static class TableColumnChecks
{
    private static void Check(bool value)
    {
        if (!value) throw new Exception("Plugin column catalog contract failed");
    }
    private static void Throws<T>(Action action) where T : Exception
    {
        try { action(); }
        catch (T) { return; }
        throw new Exception("Expected column catalog failure: " + typeof(T).Name);
    }
    // Registration is plugin-owned; enumeration must never execute a formatter just to discover metadata.
    private sealed class Plugin : IActPluginV1
    {
        internal int Calls;
        internal readonly CombatantData.ColumnDef Combatant;
        internal Plugin()
        {
            Combatant = new("同名", true, "BIGINT", "number", _ => { Calls++; throw new Exception("formatter must not run"); },
                _ => "1", (_, _) => { Calls++; throw new Exception("comparer must not run"); });
        }
        public void InitPlugin(TabPage tab, Label status)
        {
            EncounterData.ColumnDefs.Add("enc-fixture", new("Encounter label", true, "TEXT", "enc", _ => "value", _ => "sql"));
            CombatantData.ColumnDefs.Add("actor-fixture", Combatant);
            CombatantData.ColumnDefs.Add("same-label", new("同名", false, "TEXT", "other", _ => "2", _ => "2", (_, _) => 0));
            DamageTypeData.ColumnDefs.Add("category-fixture", new("Category label", true, "TEXT", "category", _ => "value", _ => "sql"));
            AttackType.ColumnDefs.Add("attack-fixture", new("Attack label", false, "TEXT", "attack", _ => "value", _ => "sql", (_, _) => 0));
            MasterSwing.ColumnDefs.Add("action-fixture", new("Action label", true, "TEXT", "action", _ => "value", _ => "sql", (_, _) => 0));
            CombatantData.OutgoingDamageTypeDataObjects.Add("fixture-out", new("duplicate label", -1, Color.Red));
            CombatantData.IncomingDamageTypeDataObjects.Add("fixture-in", new("duplicate label", 1, Color.Blue));
            ActGlobals.oFormActMain!.ValidateTableSetup();
            ActGlobals.oFormActMain.ValidateLists();
        }
        public void DeInitPlugin()
        {
            EncounterData.ColumnDefs.Remove("enc-fixture");
            CombatantData.ColumnDefs.Remove("actor-fixture");
            CombatantData.ColumnDefs.Remove("same-label");
            DamageTypeData.ColumnDefs.Remove("category-fixture");
            AttackType.ColumnDefs.Remove("attack-fixture");
            MasterSwing.ColumnDefs.Remove("action-fixture");
            CombatantData.OutgoingDamageTypeDataObjects.Remove("fixture-out");
            CombatantData.IncomingDamageTypeDataObjects.Remove("fixture-in");
        }
    }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-columns-");
        try
        {
            using (var host = new HostEnvironment(directory.FullName))
            {
                var cold = host.ReadTableColumns(TableLevel.Combatant);
                Check(cold.Revision > 0 && cold.Columns.Length == 0);
                var plugin = new Plugin();
                using var page = new TabPage();
                using var status = new Label();
                plugin.InitPlugin(page, status);
                foreach (var level in Enum.GetValues<TableLevel>())
                {
                    var catalog = host.ReadTableColumns(level);
                    Check(catalog.Columns.Any(column => column.Key.EndsWith("fixture", StringComparison.Ordinal)));
                    Check(host.ReadTableColumns(level).Revision == catalog.Revision);
                    Check(catalog.Categories.Single(category => category.Key == "fixture-out") is { Outgoing: true, AllyValue: -1 });
                    Check(catalog.Categories.Single(category => category.Key == "fixture-in") is { Outgoing: false, AllyValue: 1 });
                }
                var first = host.ReadTableColumns(TableLevel.Combatant);
                Check(first.Columns.Select(column => column.Key).SequenceEqual(new[] { "actor-fixture", "same-label" }));
                Check(first.Columns[0].Label == first.Columns[1].Label && first.Columns[0].DefaultVisible && !first.Columns[1].DefaultVisible);
                Check(first.Columns.All(column => column.CanRead && column.CanSort));
                Check(plugin.Calls == 0);
                // Caller array mutation cannot forge the next response or the cached snapshot.
                first.Columns[0] = first.Columns[1];
                first.Categories[0] = new("caller", "caller", false, 0, 0);
                Check(host.ReadTableColumns(TableLevel.Combatant).Columns[0].Key == "actor-fixture");
                MutationChecks(host, plugin);
                InvalidMetadata(host);
                plugin.DeInitPlugin();
                Check(host.ReadTableColumns(TableLevel.Combatant).Columns.Length == 0);
                plugin.InitPlugin(page, status);
                Check(host.ReadTableColumns(TableLevel.Combatant).Columns.Length == 2);
                plugin.DeInitPlugin();
                Throws<ArgumentOutOfRangeException>(() => host.ReadTableColumns((TableLevel)99));
                Task.Run(() => Throws<InvalidOperationException>(() => host.ReadTableColumns(TableLevel.Combatant))).GetAwaiter().GetResult();
            }
            using var next = new HostEnvironment(directory.FullName);
            Check(next.ReadTableColumns(TableLevel.Combatant).Columns.Length == 0);
            Console.WriteLine("Owner-thread plugin column catalogs, revisions, limits and lifecycle passed.");
        }
        finally { directory.Delete(true); }
    }
    private static void MutationChecks(HostEnvironment host, Plugin plugin)
    {
        var before = host.ReadTableColumns(TableLevel.Combatant).Revision;
        plugin.Combatant.GetCellData = _ => "changed callback";
        var changed = host.ReadTableColumns(TableLevel.Combatant).Revision;
        Check(changed != before);
        plugin.Combatant.GetCellData = null!;
        var unreadable = host.ReadTableColumns(TableLevel.Combatant);
        Check(unreadable.Revision != changed && !unreadable.Columns[0].CanRead);
        plugin.Combatant.GetCellData = _ => "restored callback";
        plugin.Combatant.SortComparer = null!;
        var missing = host.ReadTableColumns(TableLevel.Combatant);
        Check(missing.Revision != changed && !missing.Columns[0].CanSort);
        CombatantData.ColumnDefs = new(CombatantData.ColumnDefs);
        Check(host.ReadTableColumns(TableLevel.Combatant).Revision != missing.Revision);
        var categories = host.ReadTableColumns(TableLevel.Combatant).Revision;
        CombatantData.OutgoingDamageTypeDataObjects["fixture-out"] = new("changed label", -1, Color.Green);
        Check(host.ReadTableColumns(TableLevel.Combatant).Revision != categories);
        CombatantData.ColumnDefs.Remove("same-label");
        Check(host.ReadTableColumns(TableLevel.Combatant).Columns.Length == 1);
        CombatantData.ColumnDefs["same-label"] = new("同名", false, "TEXT", "other", _ => "2", _ => "2", (_, _) => 0);
    }
    private static void InvalidMetadata(HostEnvironment host)
    {
        var valid = host.ReadTableColumns(TableLevel.Combatant).Revision;
        CombatantData.ColumnDefs.Add("bad", new("\ud800", true, "TEXT", "bad", _ => "value", _ => "sql", (_, _) => 0));
        Throws<System.Text.EncoderFallbackException>(() => host.ReadTableColumns(TableLevel.Combatant));
        CombatantData.ColumnDefs.Remove("bad");
        Check(host.ReadTableColumns(TableLevel.Combatant).Revision == valid);
        CombatantData.ColumnDefs.Add("bad", null!);
        Throws<InvalidDataException>(() => host.ReadTableColumns(TableLevel.Combatant));
        CombatantData.ColumnDefs.Remove("bad");
        CombatantData.ColumnDefs.Add("bad", new(new string('x', 1025), false, "TEXT", "bad", _ => "", _ => "", (_, _) => 0));
        Throws<InvalidDataException>(() => host.ReadTableColumns(TableLevel.Combatant));
        CombatantData.ColumnDefs.Remove("bad");
        var saved = CombatantData.ColumnDefs;
        CombatantData.ColumnDefs = Enumerable.Range(0, 257).ToDictionary(index => "column" + index,
            _ => new CombatantData.ColumnDef("label", false, "TEXT", "value", _ => "", _ => "", (_, _) => 0));
        Throws<InvalidDataException>(() => host.ReadTableColumns(TableLevel.Combatant));
        CombatantData.ColumnDefs = saved;
        Check(host.ReadTableColumns(TableLevel.Combatant).Revision == valid);
        CombatantData.ColumnDefs = null!;
        Throws<InvalidDataException>(() => host.ReadTableColumns(TableLevel.Combatant));
        CombatantData.ColumnDefs = saved;
        var categories = CombatantData.OutgoingDamageTypeDataObjects;
        CombatantData.OutgoingDamageTypeDataObjects = Enumerable.Range(0, 257).ToDictionary(index => "category" + index,
            _ => new CombatantData.DamageTypeDef("label", 0, Color.Transparent));
        Throws<InvalidDataException>(() => host.ReadTableColumns(TableLevel.Combatant));
        CombatantData.OutgoingDamageTypeDataObjects = categories;
        CombatantData.ColumnDefs.Add("bad", new("bad\0label", false, "TEXT", "value", _ => "", _ => "", (_, _) => 0));
        Throws<InvalidDataException>(() => host.ReadTableColumns(TableLevel.Combatant));
        CombatantData.ColumnDefs.Remove("bad");
        Check(host.ReadTableColumns(TableLevel.Combatant).Revision == valid);
    }
}
