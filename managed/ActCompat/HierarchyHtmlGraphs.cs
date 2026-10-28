using System.Globalization;

namespace Aftercast.ActCompat
{
    // Explicit settings; no inferred plugin columns or localized terms.
    public sealed record HierarchyHtmlGraphOptions(int Window, bool Rolling, string DistributionColumn,
        string ActionSortColumn, string AllLabel, string WardPrefix,
        string[]? SelectedCombatants = null, string[]? SoloCategories = null, TableLevel[]? Levels = null);
}

namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;
    public partial class FormActMain
    {
        private string[] ReadHierarchyGraphs(HierarchyNode node, HierarchyHtmlGraphOptions options, Action check)
        {
            T Invoke<T>(Func<T> callback)
            {
                check(); Exception? failure = null;
                try { return callback(); }
                catch (Exception error) { failure = error; throw; }
                finally
                {
                    try { check(); }
                    catch (Exception after) when (failure is not null) { throw new AggregateException("Graph callback and validation failed", failure, after); }
                }
            }
            var scope = node.Capture.Scope.Level;
            var encounter = node.Capture.Guard.Encounter;
            if (scope is TableLevel.Combatant or TableLevel.Category)
            {
                var start = Invoke(() => encounter.StartTime); var end = Invoke(() => encounter.EndTime);
                if (start == DateTime.MaxValue || end == DateTime.MinValue || end <= start) return [];
                var duration = (end - start).TotalSeconds;
                if (duration > 360000) throw new InvalidDataException("Graph duration exceeds 360000 seconds");
                var seconds = (int)duration;
                if (scope == TableLevel.Combatant && seconds < 2 || scope == TableLevel.Category && (options.Rolling ? seconds : seconds / options.Window) < 2) return [];
                IEnumerable<(string, DamageTypeData?)> categories;
                if (scope == TableLevel.Combatant)
                    categories = node.Capture.Rows.Where(row => options.SelectedCombatants is null || options.SelectedCombatants.Contains(row.Combatant, StringComparer.Ordinal))
                        .Select(row => (CombatantData)row.Value).Select(actor => (actor.Name, actor.Items.TryGetValue(CombatantData.DamageTypeDataOutgoingDamage, out var category) ? category : null));
                else
                {
                    var rows = node.Capture.Rows;
                    var ordered = options.SoloCategories is null ? rows : options.SoloCategories.SelectMany(key => rows.Where(row => row.Category == key)).ToArray();
                    categories = ordered.Select(row => (DamageTypeData)row.Value).Select(category => (Invoke(() => category.Type) ?? "", (DamageTypeData?)category));
                }
                var series = new List<GraphSvg.LineSeries>();
                foreach (var (name, category) in categories)
                {
                    check();
                    if (category is null || !category.Items.TryGetValue(options.AllLabel, out var attack)) continue;
                    var actions = attack.Items.ToArray();
                    var samples = actions.Select(action => new GraphNumeric.Sample((action.Time - start).TotalSeconds, action.Damage.Number)).ToArray();
                    check();
                    var data = GraphNumeric.Line(seconds, options.Window, options.Rolling, samples);
                    series.Add(new(name, data.Plotted));
                }
                return series.Count == 0 ? [] : [GraphSvg.Lines(scope == TableLevel.Combatant ? "Encounter" : "Combatant", series, options.Rolling ? 1 : options.Window)];
            }
            if (scope == TableLevel.Attack)
            {
                var category = (DamageTypeData)node.Capture.Parents[4];
                if (!category.Items.TryGetValue(options.AllLabel, out var all)) return [];
                GraphNumeric.Share[] shares;
                if (options.DistributionColumn == "Resist")
                {
                    var actions = all.Items.Select(action => new GraphNumeric.ResistAction(action.DamageType, action.Damage.Number)).ToArray();
                    if (!actions.Any(action => action.Damage > 0)) return [];
                    shares = GraphNumeric.Resist(actions, options.WardPrefix, options.AllLabel);
                }
                else
                {
                    var binding = SelectTableColumn(node.Capture.Columns, options.DistributionColumn, true);
                    if (binding.Sql is not AttackType.StringDataCallback sql || binding.Sort is not Comparison<AttackType> compare) throw new InvalidDataException("Graph column requires SQL and sorting callbacks");
                    double Parse(AttackType attack) => double.Parse(Invoke(() => sql(attack)), NumberStyles.Any, CultureInfo.GetCultureInfo("en-US"));
                    var denominator = Parse(all); var attacks = node.Capture.Rows.Select(row => (AttackType)row.Value).ToList();
                    attacks.Sort((left,right) => Invoke(() => compare(left,right))); attacks.Reverse();
                    var values = new List<GraphNumeric.CategoryValue>();
                    foreach (var attack in attacks)
                    {
                        if (ReferenceEquals(attack, all)) continue;
                        check(); string? raw = null; Exception? failure = null;
                        try { raw = sql(attack); } catch (Exception error) { failure = error; }
                        try { check(); }
                        catch (Exception after) when (failure is not null) { throw new AggregateException("Graph SQL callback and validation failed", failure, after); }
                        if (failure is null && double.TryParse(raw, NumberStyles.Any, CultureInfo.GetCultureInfo("en-US"), out var parsed))
                            values.Add(new(Invoke(() => attack.Type) ?? "", parsed));
                    }
                    shares = GraphNumeric.Distribution(denominator, values);
                }
                check(); return [GraphSvg.Distribution(Invoke(() => category.Type) ?? "", shares)];
            }
            if (scope == TableLevel.Action)
            {
                var attack = (AttackType)node.Capture.Parents[6];
                var binding = SelectTableColumn(node.Capture.Columns, options.ActionSortColumn, true);
                if (binding.Sort is not Comparison<MasterSwing> compare) throw new InvalidDataException("Graph action column requires a sorting callback");
                var entries = new Dictionary<GraphNumeric.BarAction, MasterSwing>(ReferenceEqualityComparer.Instance);
                foreach (var row in node.Capture.Rows)
                {
                    var action = (MasterSwing)row.Value;
                    entries.Add(new(action.AttackType, action.Damage.Number, action.Time), action);
                }
                var bars = GraphNumeric.ActionBars(entries.Keys, (left,right) => Invoke(() => compare(entries[left],entries[right])),
                    (left,right) => entries[left].TimeSorter.CompareTo(entries[right].TimeSorter));
                check(); return [GraphSvg.Bars(Invoke(() => attack.Type) ?? "", bars)];
            }
            return [];
        }
    }
}
