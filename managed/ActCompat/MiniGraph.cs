using System.Diagnostics;

namespace Aftercast.ActCompat
{
    public sealed record MiniGraphRowDisplay(string Label, string TooltipValue, bool IsSelf);
    public sealed record MiniGraphDisplay(string AxisLabel, string TotalsText, MiniGraphRowDisplay[] Rows);
    public sealed record MiniGraphData(ulong EncounterId, string Column, bool NoActors, GraphNumeric.SimpleResult? Data, MiniGraphDisplay? Display);
}
namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;
    public partial class FormActMain
    {
        private static void ValidateMiniGraphOutput(GraphNumeric.SimpleResult? data)
        {
            if (data is null) return;
            var bytes = 0;
            var utf8 = new System.Text.UTF8Encoding(false, true);
            foreach (var item in data.Values)
            {
                var nameBytes = utf8.GetByteCount(item.Name);
                if (item.Name.Length > 4096 || nameBytes > 8192) throw new InvalidDataException("Mini graph label exceeds limit");
                // JSON can escape every byte as six ASCII characters; include row overhead.
                bytes = checked(bytes + 6 * nameBytes + 320);
                if (bytes > 3 * 1024 * 1024) throw new InvalidDataException("Mini graph response exceeds limit");
            }
        }
        internal static MiniGraphDisplay? FormatMiniGraph(GraphNumeric.SimpleResult? data, string? character)
        {
            if (data is null) return null;
            static long Truncate(double value)
            {
                // ACT converts labels to Int64. Avoid undefined or platform-specific conversion for unrepresentable values.
                if (!double.IsFinite(value) || value < 0 || value >= 9223372036854775808.0)
                    throw new InvalidDataException("Mini graph label exceeds Int64 range");
                return (long)value;
            }
            var rows = data.Values.Select(item => new MiniGraphRowDisplay(
                DeathNumericTooltip(Truncate(item.Value)),
                item.Value.ToString("#,0", System.Globalization.CultureInfo.CurrentCulture),
                string.Equals(item.Name, character, StringComparison.Ordinal))).ToArray();
            return new(DeathNumericTooltip(Truncate(data.Axis)),
                string.Format(System.Globalization.CultureInfo.CurrentCulture, "Total: {0:0,0}\nAVG: {1:0,0.00}", data.Total, data.Average), rows);
        }
        // This capture is local to the call: periodic mini refresh must not invalidate a user's report capture.
        internal MiniGraphData ReadMiniGraph(ulong encounterId, string column, bool onlyAllies)
        {
            EnsureTextReportOwner();
            if (encounterId == 0 || string.IsNullOrWhiteSpace(column) || column.Length > 256 || column.Any(char.IsControl))
                throw new ArgumentException("Invalid mini graph encounter or sorting column");
            lock (CombatGate)
            {
                var clock = Stopwatch.StartNew(); var errors = OwnerErrors.Item1; var selectedRevision = selectiveRevision;
                var revision = ReadTableColumns(TableLevel.Combatant).Revision;
                void Guard()
                {
                    if (clock.Elapsed > TimeSpan.FromSeconds(2)) throw new TimeoutException("Mini graph exceeded two seconds");
                    if (OwnerErrors.Item1 != errors) throw new InvalidOperationException("Mini graph callback reported an owner-thread error");
                    if (selectiveRevision != selectedRevision) throw new InvalidOperationException("Mini graph selection changed");
                    RequireTableRevision(TableLevel.Combatant, revision);
                }
                activeTableQuery = true;
                try
                {
                    var tables = 0; var rows = 0; var captures = new List<ReportCapture>();
                    var node = BuildHierarchy(new(TableLevel.Combatant, encounterId), TableLevel.Combatant, ref tables, ref rows, captures, Guard);
                    void Check() => CheckHierarchy(node, Guard);
                    Check();
                    var options = new HierarchyHtmlGraphOptions(1, false, "", "", "", "", Simple: true, SimpleColumn: column, OnlyAllies: onlyAllies);
                    var (data, noActors) = ReadSimpleGraph(node, options, Check);
                    ValidateMiniGraphOutput(data);
                    Check();
                    return new(encounterId, column, noActors, data, FormatMiniGraph(data, ActGlobals.charName));
                }
                finally { activeTableQuery = false; }
            }
        }
    }
}
