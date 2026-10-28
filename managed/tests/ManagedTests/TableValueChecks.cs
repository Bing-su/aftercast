using System.Drawing;
using System.Globalization;
using System.Runtime.CompilerServices;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class TableValueChecks
{
    private static void Check(bool condition, [CallerArgumentExpression(nameof(condition))] string? expression = null)
    {
        if (!condition) throw new Exception("Plugin table values contract failed: " + expression);
    }
    private static void Throws<T>(Action action) where T : Exception
    {
        try { action(); }
        catch (T) { return; }
        throw new Exception("Expected table query failure: " + typeof(T).Name);
    }
    private sealed class Plugin : IActPluginV1
    {
        private readonly int owner = Environment.CurrentManagedThreadId;
        internal readonly HashSet<object> Known = new(ReferenceEqualityComparer.Instance);
        internal Action? OnCell;
        internal int Calls;
        private FormActMain root = null!;
        internal void Own(object value)
        {
            Check(Environment.CurrentManagedThreadId == owner && Known.Contains(value));
            Calls++;
        }
        private string Text(object value) { Own(value); OnCell?.Invoke(); return value.GetType().Name; }
        private Color Foreground(object value) { Own(value); return Color.Red; }
        public void InitPlugin(TabPage tab, Label status)
        {
            root = ActGlobals.oFormActMain!;
            CombatFixture.ConfigureCategories();
            root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
            root.BeforeLogLineRead += Read;
            EncounterData.ColumnDefs["fixture"] = new("fixture", true, "TEXT", "value", value => Text(value), _ => "sql") { GetCellForeColor = value => Foreground(value) };
            CombatantData.ColumnDefs["fixture"] = new("fixture", true, "TEXT", "value", value => Text(value), _ => "sql", (left, right) => { Own(left); Own(right); return left.Damage.CompareTo(right.Damage); }) { GetCellForeColor = value => Foreground(value) };
            DamageTypeData.ColumnDefs["fixture"] = new("fixture", true, "TEXT", "value", value => Text(value), _ => "sql") { GetCellForeColor = value => Foreground(value) };
            AttackType.ColumnDefs["fixture"] = new("fixture", true, "TEXT", "value", value => Text(value), _ => "sql", (left, right) => { Own(left); Own(right); return left.Damage.CompareTo(right.Damage); }) { GetCellForeColor = value => Foreground(value) };
            MasterSwing.ColumnDefs["fixture"] = new("fixture", true, "TEXT", "value", value => Text(value), _ => "sql", (left, right) => { Own(left); Own(right); return left.TimeSorter.CompareTo(right.TimeSorter); }) { GetCellForeColor = value => Foreground(value) };
            CombatantData.ColumnDefs["name"] = new("name", true, "TEXT", "name", value => { Own(value); return value.Name; }, _ => "sql", (left, right) => StringComparer.Ordinal.Compare(left.Name, right.Name));
            CombatantData.ColumnDefs["damage"] = new("damage", true, "BIGINT", "damage", value => value.Damage.ToString(CultureInfo.InvariantCulture), _ => "sql", (left, right) => left.Damage.CompareTo(right.Damage));
        }
        private void Read(bool import, LogLineEventArgs info)
        {
            Check(import);
            var parts = info.originalLogLine.Split('|');
            root.SetEncounter(info.detectedTime, parts[2], "Target");
            root.AddCombatAction(new(1, false, "None", new Dnum(long.Parse(parts[1], CultureInfo.InvariantCulture)), info.detectedTime,
                root.GlobalTimeSorter, parts[3], parts[2], "Physical", "Target"));
        }
        public void DeInitPlugin()
        {
            root.BeforeLogLineRead -= Read;
            EncounterData.ColumnDefs.Remove("fixture");
            CombatantData.ColumnDefs.Remove("fixture");
            CombatantData.ColumnDefs.Remove("name");
            CombatantData.ColumnDefs.Remove("damage");
            DamageTypeData.ColumnDefs.Remove("fixture");
            AttackType.ColumnDefs.Remove("fixture");
            MasterSwing.ColumnDefs.Remove("fixture");
        }
    }
    private sealed class Fixture : IDisposable
    {
        internal readonly DirectoryInfo Directory = System.IO.Directory.CreateTempSubdirectory("aftercast-table-values-");
        internal readonly HostEnvironment Host;
        internal readonly Plugin Plugin = new();
        internal readonly FormActMain Root;
        internal readonly EncounterData Encounter;
        internal readonly ulong Id;
        private readonly TabPage page = new();
        private readonly Label label = new();
        internal Fixture()
        {
            Host = new(Directory.FullName);
            Root = ActGlobals.oFormActMain!;
            Root.ChangeZone("Live");
            Plugin.InitPlugin(page, label);
            ActGlobals.charName = "A";
            var source = Path.Combine(Directory.FullName, "source.log");
            File.WriteAllText(source, "0|100|A|Hit\n0|100|B|Hit\n1|200|A|Other\n2|100|B|Hit\n");
            Check(Host.ImportLog(source).Damage == 500);
            Id = Host.ListEncounters(0, 16).Items.Single().Id;
            Encounter = Root.ZoneList[0].Items.Single();
            Plugin.Known.Add(Encounter);
            foreach (var actor in Encounter.Items.Values)
            {
                Plugin.Known.Add(actor);
                foreach (var category in actor.Items.Values)
                {
                    Plugin.Known.Add(category);
                    foreach (var attack in category.Items.Values)
                    {
                        Plugin.Known.Add(attack);
                        foreach (var action in attack.Items) Plugin.Known.Add(action);
                    }
                }
            }
        }
        internal TableQuery Query(TableScope? scope = null, string[]? columns = null, TableSort? sort = null) =>
            new(scope ?? new(TableLevel.Combatant, Id), Host.ReadTableColumns((scope ?? new(TableLevel.Combatant, Id)).Level).Revision,
                columns ?? ["fixture"], Sort: sort);
        internal TablePageData Read(TableQuery query) => Host.ReadTable(query) ?? throw new Exception("Expected retained table scope");
        public void Dispose()
        {
            try { Plugin.DeInitPlugin(); }
            finally { page.Dispose(); label.Dispose(); Host.Dispose(); Directory.Delete(true); }
        }
    }
    internal static void Run()
    {
        using var fixture = new Fixture();
        LevelsAndOrdering(fixture);
        MainDefaultOrdering(fixture);
        ZoneScope(fixture);
        CellFailures(fixture);
        ReentryAndMutation(fixture);
        SortingFailures(fixture);
        LimitsAndValidation(fixture);
        ComparisonBudget(fixture);
        Console.WriteLine("Original-object owner-thread table cells/colors, bounded sorting, invalidation and failures passed.");
    }
    private static void ZoneScope(Fixture fixture)
    {
        var scope = new TableScope(TableLevel.Encounter, fixture.Id, ZoneAnchor: true);
        var query = fixture.Query(scope);
        Check(fixture.Read(query).Total == 1);
        Check(fixture.Host.ListZoneEncounters(fixture.Id, 0, 1)!.Total == 1);
        Throws<ArgumentException>(() => fixture.Host.ReadTable(fixture.Query(new TableScope(TableLevel.Encounter, fixture.Id))));
        var zone = fixture.Root.ZoneList[0];
        Check(zone.Items.Remove(fixture.Encounter));
        try { Check(fixture.Host.ReadTable(query) is null && fixture.Host.ListZoneEncounters(fixture.Id, 0, 1) is null); }
        finally { zone.Items.Add(fixture.Encounter); }
    }

    private static void MainDefaultOrdering(Fixture fixture)
    {
        var query = fixture.Query(columns: ["name"]);
        var ordinary = fixture.Read(query);
        var descending = fixture.Read(query with { MainDescending = true });
        var ascending = fixture.Read(query with { MainDescending = false });
        Check(descending.Rows[0].Combatant == "A" && ascending.Rows[^1].Combatant == "A");
        Check(ordinary.Rows.Select(row => row.Index).SequenceEqual(Enumerable.Range(0, ordinary.Total)));
        Check(descending.Rows.Select(row => row.Index).SequenceEqual(ascending.Rows.Select(row => row.Index).Reverse()));
        int calls = 0;
        CombatantData.ColumnDefs["EncDPS"] = new("ACT default", true, "TEXT", "value", value => value.Name, _ => "sql",
            (left, right) => { calls++; return StringComparer.Ordinal.Compare(left.Name, right.Name); });
        var registered = fixture.Read(fixture.Query(columns: ["name"]) with { MainDescending = false });
        Check(calls > 0 && registered.Rows[0].Combatant == "A");
        CombatantData.ColumnDefs["EncDPS"] = new("bad", true, "TEXT", "value", value => value.Name, _ => "sql",
            (_, _) => throw new InvalidOperationException("synthetic comparison"));
        var failed = fixture.Read(fixture.Query(columns: ["name"]) with { MainDescending = true });
        Check(failed.SortError == "sort-callback-error" && failed.Rows.Select(row => row.Index).SequenceEqual(Enumerable.Range(0, failed.Total)));
        CombatantData.ColumnDefs.Remove("EncDPS");
        var ordering = typeof(HostEnvironment).Assembly.GetType("Aftercast.ActCompat.TableOrdering", true)!;
        var method = ordering.GetMethod("Order", System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.NonPublic)!;
        foreach (int count in new[] { 3, 24 })
        {
            var expected = Enumerable.Range(0, count).ToList();
            expected.Sort((_, _) => 0); expected.Reverse();
            object?[] args = [count, (Func<int, int, int>)((_, _) => 0), true, (Action<Exception>)(_ => { }), null, true];
            var actual = (int[])method.Invoke(null, args)!;
            Check(args[4] is null && actual.SequenceEqual(expected));
            if (count == 24) Check(string.Join(",", actual) == "23,22,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,0");
        }
    }

    private static void LevelsAndOrdering(Fixture fixture)
    {
        var scopes = new[] { new TableScope(TableLevel.Encounter), new(TableLevel.Combatant, fixture.Id),
            new(TableLevel.Category, fixture.Id, "A"), new(TableLevel.Attack, fixture.Id, "A", "Damage"),
            new(TableLevel.Action, fixture.Id, "A", "Damage", "All") };
        foreach (var scope in scopes)
        {
            var page = fixture.Read(fixture.Query(scope));
            Check(page.Total > 0 && page.Rows.Length > 0);
            Check(page.Rows.All(row => row.Cells.Single() is { Error: null, ForegroundArgb: var color, BackgroundArgb: null } && color == Color.Red.ToArgb()));
            if (scope.Level is TableLevel.Combatant or TableLevel.Attack or TableLevel.Action)
                Check(fixture.Read(fixture.Query(scope, sort: new("fixture"))).SortError is null);
        }
        Check(fixture.Read(fixture.Query(new(TableLevel.Category, fixture.Id, "Target"))).Rows.All(row => row.Combatant == "TARGET"));
        var selected = fixture.Read(fixture.Query(columns: ["damage", "name"], sort: new("damage")));
        Check(selected.Columns.Select(column => column.Key).SequenceEqual(new[] { "damage", "name" }));
        Check(selected.Rows.Select(row => row.Cells[0].Text).SequenceEqual(new[] { "0", "200", "300" }));
        var descending = fixture.Read(fixture.Query(columns: ["damage"], sort: new("damage", Descending: true)));
        Check(descending.Rows.Select(row => row.Cells[0].Text).SequenceEqual(new[] { "300", "200", "0" }));
        Check(fixture.Read(fixture.Query(columns: ["damage"], sort: new("damage")) with { Offset = 1, Limit = 1 }).Rows[0].Cells[0].Text == "200");
        var definition = CombatantData.ColumnDefs["fixture"];
        var original = definition.SortComparer;
        definition.SortComparer = (_, _) => 0;
        var natural = fixture.Read(fixture.Query());
        var tied = fixture.Read(fixture.Query(sort: new("fixture", Descending: true)));
        Check(tied.Rows.Select(row => row.Index).SequenceEqual(natural.Rows.Select(row => row.Index)));
        Check(fixture.Read(fixture.Query(columns: ["name"], sort: new("fixture", "name"))).Rows.Select(row => row.Cells[0].Text).SequenceEqual(new[] { "A", "B", "Target" }));
        definition.SortComparer = (left, right) => left.Damage == right.Damage ? 0 : left.Damage < right.Damage ? int.MinValue : int.MaxValue;
        Check(fixture.Read(fixture.Query(sort: new("fixture", Descending: true))).SortError is null);
        definition.SortComparer = original;
        var sql = definition.GetSqlData;
        definition.GetSqlData = _ => throw new Exception("cell queries must not execute SQL callbacks");
        Check(fixture.Read(fixture.Query()).Rows.All(row => row.Cells[0].Error is null));
        definition.GetSqlData = sql;
        var query = fixture.Query();
        var pageCopy = fixture.Read(query);
        pageCopy.Rows[0].Cells[0] = new("caller", null, null, null);
        Check(fixture.Read(query).Rows[0].Cells[0].Text != "caller");
        Check(fixture.Host.ReadTable(query with { Scope = new(TableLevel.Combatant, ulong.MaxValue) }) is null);
        Check(fixture.Host.ReadTable(fixture.Query(new(TableLevel.Action, fixture.Id, "A", "Damage", "missing"))) is null);
    }
    private static void CellFailures(Fixture fixture)
    {
        var definition = CombatantData.ColumnDefs["fixture"];
        var text = definition.GetCellData;
        var fore = definition.GetCellForeColor;
        var back = definition.GetCellBackColor;
        foreach (CombatantData.StringDataCallback? bad in new CombatantData.StringDataCallback?[] {
            _ => throw new InvalidOperationException("synthetic formatter failure"), _ => null!, _ => "\ud800", _ => "NUL\0", _ => new string('x', 4097), null })
        {
            definition.GetCellData = bad!;
            var before = fixture.Host.Diagnostics.Item1;
            var page = fixture.Read(fixture.Query(columns: ["fixture", "damage"]));
            Check(page.Rows.All(row => row.Cells[0].Error == "column-error" && row.Cells[0].Text is null && row.Cells[1].Error is null));
            Check(fixture.Host.Diagnostics.Item1 == before + 1);
        }
        definition.GetCellData = text;
        definition.GetCellForeColor = _ => throw new InvalidOperationException("synthetic color failure");
        Check(fixture.Read(fixture.Query()).Rows.All(row => row.Cells[0].Error == "column-error"));
        definition.GetCellForeColor = fore;
        definition.GetCellBackColor = null!;
        Check(fixture.Read(fixture.Query()).Rows.All(row => row.Cells[0].Error == "column-error"));
        definition.GetCellBackColor = _ => Color.FromArgb(0, 0, 0, 0);
        Check(fixture.Read(fixture.Query()).Rows.All(row => row.Cells[0].BackgroundArgb == 0));
        definition.GetCellBackColor = back;
    }
    private static void ReentryAndMutation(Fixture fixture)
    {
        var query = fixture.Query();
        fixture.Plugin.OnCell = () => fixture.Host.ReadTable(query);
        Check(fixture.Read(query).Rows.All(row => row.Cells[0].Error == "column-error"));
        fixture.Plugin.OnCell = () => fixture.Root.ChangeZone("forbidden mutation");
        Check(fixture.Read(query).Rows.All(row => row.Cells[0].Error == "column-error"));
        Check(fixture.Root.CurrentZone == "Live");
        var keys = new[] { "fixture", "damage" };
        query = fixture.Query(columns: keys);
        fixture.Plugin.OnCell = () => keys[1] = "unknown key";
        Check(fixture.Read(query).Columns[1].Key == "damage");
        var old = CombatantData.ColumnDefs["name"];
        var wrongCalls = 0;
        query = fixture.Query(columns: ["fixture", "name"]);
        fixture.Plugin.OnCell = () => CombatantData.ColumnDefs["name"] = new("replacement", true, "TEXT", "name", _ => { wrongCalls++; return "wrong"; }, _ => "sql", (_, _) => 0);
        Throws<InvalidOperationException>(() => fixture.Read(query));
        Check(wrongCalls == 0);
        fixture.Plugin.OnCell = null;
        CombatantData.ColumnDefs["name"] = old;
        Throws<InvalidOperationException>(() => fixture.Read(query));
        query = fixture.Query();
        fixture.Plugin.OnCell = () => CombatantData.ColumnDefs.Remove("fixture");
        var definition = CombatantData.ColumnDefs["fixture"];
        Throws<InvalidOperationException>(() => fixture.Read(query));
        fixture.Plugin.OnCell = null;
        CombatantData.ColumnDefs["fixture"] = definition;
        query = fixture.Query();
        fixture.Plugin.OnCell = () => fixture.Root.ZoneList[0].Items.Clear();
        Throws<InvalidOperationException>(() => fixture.Read(query));
        fixture.Plugin.OnCell = null;
        fixture.Root.ZoneList[0].Items.Add(fixture.Encounter);
        var calls = fixture.Plugin.Calls;
        Throws<InvalidOperationException>(() => fixture.Read(query with { Revision = query.Revision + 1 }));
        Check(fixture.Plugin.Calls == calls);
    }
    private static void SortingFailures(Fixture fixture)
    {
        var definition = CombatantData.ColumnDefs["fixture"];
        var original = definition.SortComparer;
        var natural = fixture.Read(fixture.Query()).Rows.Select(row => row.Index).ToArray();
        foreach (Comparison<CombatantData> bad in new Comparison<CombatantData>[] {
            (_, _) => throw new InvalidOperationException("synthetic comparison failure"), (_, _) => 1,
            (left, right) => left.Name == right.Name ? 0 : 1,
            (left, right) => left.Name == right.Name ? 0 : (left.Name, right.Name) is ("A", "B") or ("B", "Target") or ("Target", "A") ? -1 : 1 })
        {
            definition.SortComparer = bad;
            var page = fixture.Read(fixture.Query(sort: new("fixture")));
            Check(page.SortError is not null && page.Rows.Select(row => row.Index).SequenceEqual(natural));
        }
        var pairCalls = new Dictionary<(string, string), int>();
        definition.SortComparer = (left, right) => {
            if (ReferenceEquals(left, right)) return 0;
            var key = StringComparer.Ordinal.Compare(left.Name, right.Name) < 0 ? (left.Name, right.Name) : (right.Name, left.Name);
            pairCalls.TryGetValue(key, out var count); pairCalls[key] = ++count;
            return Math.Sign(StringComparer.Ordinal.Compare(left.Name, right.Name)) * (count <= 2 ? 1 : -1);
        };
        Check(fixture.Read(fixture.Query(sort: new("fixture"))).SortError == "inconsistent-comparison");
        definition.SortComparer = (left, right) => { CombatantData.ColumnDefs.Remove("fixture"); return 0; };
        Throws<InvalidOperationException>(() => fixture.Read(fixture.Query(sort: new("fixture"))));
        CombatantData.ColumnDefs["fixture"] = definition;
        definition.SortComparer = original;
    }
    private sealed class EqualityTrap(string name, EncounterData parent) : CombatantData(name, parent)
    {
        public override bool Equals(object? other) => throw new Exception("Table identity checks must use reference equality");
        public override int GetHashCode() => throw new Exception("Table identity checks must use reference equality");
    }
    private static void LimitsAndValidation(Fixture fixture)
    {
        var query = fixture.Query();
        var calls = fixture.Plugin.Calls;
        foreach (var invalid in new[] { query with { Offset = -1 }, query with { Limit = 257 }, query with { Revision = 0 },
            query with { Columns = [] }, query with { Columns = ["fixture", "fixture"] }, query with { Columns = ["unknown"] },
            query with { Scope = new(TableLevel.Combatant) }, query with { Scope = new(TableLevel.Encounter, fixture.Id) } })
            Throws<ArgumentException>(() => fixture.Read(invalid));
        Throws<ArgumentException>(() => fixture.Read(fixture.Query(new(TableLevel.Encounter), sort: new("fixture"))));
        Check(fixture.Plugin.Calls == calls);
        Task.Run(() => Throws<InvalidOperationException>(() => fixture.Read(query))).GetAwaiter().GetResult();
        var actions = fixture.Encounter.Items["A"].Items["Damage"].Items["All"].Items;
        var saved = actions.ToArray();
        actions.AddRange(Enumerable.Repeat(saved[0], 16385));
        Throws<InvalidDataException>(() => fixture.Read(fixture.Query(new(TableLevel.Action, fixture.Id, "A", "Damage", "All"))));
        actions.Clear(); actions.AddRange(saved);
        var text = new string('x', 4096);
        var keys = Enumerable.Range(0, 32).Select(index => "bulk" + index).ToArray();
        foreach (var key in keys) CombatantData.ColumnDefs[key] = new(key, false, "TEXT", key, _ => text, _ => "sql", (_, _) => 0);
        var added = Enumerable.Range(0, 65).Select(index => "extra" + index).ToArray();
        foreach (var key in added) fixture.Encounter.Items[key] = new(key, fixture.Encounter);
        Throws<InvalidDataException>(() => fixture.Read(fixture.Query(columns: keys) with { Limit = 256 }));
        foreach (var key in added) fixture.Encounter.Items.Remove(key);
        foreach (var key in keys) CombatantData.ColumnDefs.Remove(key);
        var trap = new EqualityTrap("Trap", fixture.Encounter);
        fixture.Encounter.Items["TRAP"] = trap;
        fixture.Plugin.Known.Add(trap);
        Check(fixture.Read(fixture.Query()).Rows.Length == 4);
        fixture.Encounter.Items.Remove("TRAP");
        // Invalid unrelated metadata must also abort publication, not escape as a partial page.
        CombatantData.ColumnDefs["bad"] = null!;
        Throws<InvalidDataException>(() => fixture.Read(query));
        CombatantData.ColumnDefs.Remove("bad");
    }
    private static void ComparisonBudget(Fixture fixture)
    {
        var actions = fixture.Encounter.Items["A"].Items["Damage"].Items["All"].Items;
        var saved = actions.ToArray();
        actions.Clear();
        try
        {
            for (var index = 0; index < 16384; index++)
            {
                var action = new MasterSwing(1, false, "None", new Dnum(1), DateTime.UnixEpoch,
                    index * 7919 % 16384, "Budget", "A", "Physical", "Target");
                actions.Add(action);
                fixture.Plugin.Known.Add(action);
            }
            var scope = new TableScope(TableLevel.Action, fixture.Id, "A", "Damage", "All");
            var result = fixture.Read(fixture.Query(scope, sort: new("fixture")));
            Check(result.SortError == "comparison-budget");
            Check(result.Rows.Select(row => row.Index).SequenceEqual(Enumerable.Range(0, 16)));
            Check(fixture.Read(fixture.Query(scope) with { Offset = 16383, Limit = 1 }).Rows.Single().Index == 16383);
        }
        finally { actions.Clear(); actions.AddRange(saved); }
    }
}
