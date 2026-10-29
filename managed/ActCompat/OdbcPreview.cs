using System.Globalization;
using System.Diagnostics;
using System.Text.RegularExpressions;

namespace Aftercast.ActCompat
{
    // Offline data only. These records do not expose any database or network operation.
    public sealed record OdbcPreviewColumn(string Name, string SqlType);
    public sealed record OdbcPreview(ulong Revision, string Table, OdbcPreviewColumn[] Columns,
        string CreateSql, string InsertSql, string[][] Parameters);
}

namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;
    public partial class FormActMain
    {
        internal OdbcPreview PreviewOdbcTable(ulong revision, string[] destinationColumns, bool encounter = false)
        {
            EnsureTextReportOwner(); ArgumentNullException.ThrowIfNull(destinationColumns);
            var names = destinationColumns.ToArray();
            if (names.Length is < 1 or > MaximumColumns || names.Any(string.IsNullOrEmpty) ||
                names.Distinct(StringComparer.CurrentCultureIgnoreCase).Count() != names.Length)
                throw new ArgumentException("Invalid synthetic destination columns");
            lock (CombatGate)
            {
                var capture = tableReportCapture;
                if (capture is null || revision == 0 || revision != capture.Revision)
                    throw new InvalidOperationException("Report selection revision mismatch");
                CheckTableReport(capture);
                if (encounter && capture.Scope.Level != TableLevel.Combatant)
                    throw new ArgumentException("Encounter preview requires an encounter-owned combatant snapshot");
                var level = encounter ? TableLevel.Encounter : capture.Scope.Level;
                var catalog = encounter ? ReadTableColumns(level) : null;
                var snapshot = encounter ? columnSnapshots[level].Snapshot : capture.Columns;
                var columnRevision = encounter ? catalog!.Revision : capture.ColumnRevision;
                var table = level switch
                {
                    TableLevel.Encounter => "encounter_table", TableLevel.Combatant => "combatant_table", TableLevel.Category => "damagetype_table",
                    TableLevel.Attack => "attacktype_table", TableLevel.Action => "swing_table",
                    _ => throw new ArgumentException("Unsupported synthetic ODBC scope")
                };
                var selected = new List<ColumnBinding>();
                foreach (var name in names)
                {
                    ColumnText(name);
                    var matches = snapshot.Bindings.Where(c => string.Equals(c.Metadata.SqlDataName, name,
                        StringComparison.CurrentCultureIgnoreCase)).ToArray();
                    if (matches.Length != 1 || matches[0].Sql is null)
                        throw new InvalidDataException("Destination column is missing, ambiguous, or has no SQL callback");
                    selected.Add(matches[0]);
                }
                var columns = selected.Select((c, index) => new OdbcPreviewColumn(names[index],
                    ValidateSyntheticSqlType(c.Metadata.SqlDataType))).ToArray();
                static string Quote(string value) => "\"" + value.Replace("\"", "\"\"", StringComparison.Ordinal) + "\"";
                var schema = $"CREATE TABLE {Quote(table)} ({string.Join(", ", columns.Select(c => Quote(c.Name.ToLower(CultureInfo.CurrentCulture)) + " " + c.SqlType))});";
                var insert = $"INSERT INTO {Quote(table)} ({string.Join(", ", columns.Select(c => Quote(c.Name)))}) VALUES ({string.Join(", ", columns.Select(_ => "?"))});";
                activeTableQuery = true;
                try
                {
                    // Read every value before returning the complete plan; failures cannot publish a partial plan.
                    string[][] rows;
                    if (encounter)
                    {
                        var clock = Stopwatch.StartNew(); var errors = OwnerErrors.Item1; var bytes = 0;
                        void Check()
                        {
                            if (clock.Elapsed > TimeSpan.FromSeconds(2)) throw new TimeoutException("ODBC preview exceeded cooperative two-second budget");
                            CheckTableReport(capture); RequireTableRevision(level, columnRevision);
                            if (OwnerErrors.Item1 != errors) throw new InvalidOperationException("ODBC callback reported an owner-thread exception");
                        }
                        var row = new List<string>();
                        foreach (var column in selected)
                        {
                            Check(); RequireTableBinding(level, snapshot, column); Exception? failure = null;
                            try
                            {
                                var value = ((EncounterData.StringDataCallback)column.Sql!)(capture.Guard.Encounter);
                                if (value is null || value.Contains('\0') || ColumnUtf8.GetByteCount(value) > MaximumCellBytes)
                                    throw new InvalidDataException("Invalid or oversized SQL value");
                                bytes += ColumnUtf8.GetByteCount(value);
                                if (bytes > MaximumTableTextBytes) throw new InvalidDataException("ODBC preview text exceeds 8 MiB");
                                row.Add(SyntheticSqlValue(value));
                            }
                            catch (Exception error) { failure = error; throw; }
                            finally
                            {
                                try { Check(); RequireTableBinding(level, snapshot, column); }
                                catch (Exception after) when (failure is not null) { throw new AggregateException("ODBC callback and validation failed", failure, after); }
                            }
                        }
                        Check(); rows = [row.ToArray()];
                    }
                    else
                    {
                        var values = ReadReportValues(capture, TableReportFormat.TableXml, [], selected.ToArray(), null, null, null, validateXml:false);
                        rows = values.Rows.Select(row => row.Select(SyntheticSqlValue).ToArray()).ToArray();
                        values.Check();
                    }
                    return new(revision, table, columns, schema, insert, rows);
                }
                finally { activeTableQuery = false; }
            }
        }
        private static string ValidateSyntheticSqlType(string value)
        {
            ColumnText(value);
            if (!Regex.IsMatch(value, @"\A(?:TINYINT|SMALLINT|INTEGER|INT|BIGINT|REAL|FLOAT|DOUBLE|DOUBLE PRECISION|NUMERIC|DECIMAL|CHAR|VARCHAR|TEXT|DATE|TIME|TIMESTAMP|DATETIME|BOOLEAN|BIT|BLOB|BINARY|VARBINARY|LONGVARCHAR|LONGVARBINARY)(?:\([0-9]{1,4}(?:,[0-9]{1,4})?\))?\z", RegexOptions.CultureInvariant | RegexOptions.IgnoreCase, TimeSpan.FromMilliseconds(50)))
                throw new InvalidDataException("Unsupported synthetic SQL type");
            return value;
        }
        private static string SyntheticSqlValue(string value) => value
            .Replace(float.NaN.ToString(CultureInfo.CurrentCulture), "0", StringComparison.Ordinal)
            .Replace(float.NegativeInfinity.ToString(CultureInfo.CurrentCulture), "0", StringComparison.Ordinal)
            .Replace(float.PositiveInfinity.ToString(CultureInfo.CurrentCulture), "0", StringComparison.Ordinal);
    }
}
