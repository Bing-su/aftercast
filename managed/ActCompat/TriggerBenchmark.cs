using System.Diagnostics;
using System.Text;
using System.Text.RegularExpressions;
using Aftercast.ActCompat;

namespace Aftercast.ActCompat
{
    public sealed record TriggerBenchmarkRow(string Key, string Category, bool Restricted, long Milliseconds, int Matches);
    public sealed record TriggerBenchmarkResult(int Lines, long Dropped, int TimestampLength, long BaselineMilliseconds, TriggerBenchmarkRow[] Rows);
}
namespace Advanced_Combat_Tracker
{
    public partial class FormActMain
    {
        // Read-only timing: never calls ProcessCustomTriggers or invokes its effects.
        internal TriggerBenchmarkResult BenchmarkTriggers(ulong revision, ulong snapshotRevision, string[] keys)
        {
            ArgumentNullException.ThrowIfNull(keys);
            if (keys.Length is < 1 or > 32 || keys.Any(string.IsNullOrEmpty) || keys.Distinct(StringComparer.Ordinal).Count() != keys.Length)
                throw new ArgumentException("Select 1–32 distinct triggers");
            (string Key, CustomTrigger Trigger)[] selected;
            lock (triggerGate)
            {
                CheckTriggerRevision(revision);
                selected = keys.Select(key => (key, CustomTriggers.TryGetValue(key, out var trigger) ? trigger : throw new InvalidOperationException("Selected trigger is unavailable"))).ToArray();
            }
            string[] lines; long dropped; int timestamp;
            lock (CombatGate)
            {
                ObjectDisposedException.ThrowIf(actionsClosed, this);
                if (actionSnapshot is not { } snapshot || snapshot.Revision != snapshotRevision) throw new InvalidOperationException("Log snapshot is stale");
                if (snapshot.Logs.Page.Total <= 1000) throw new ArgumentException("Benchmark requires more than 1000 retained log entries");
                long bytes = 0;
                lines = snapshot.Logs.Page.Items.Select(item => { bytes += Encoding.UTF8.GetByteCount(item.Text); if (bytes > 16 * 1024 * 1024) throw new InvalidDataException("Benchmark exceeds 16 MiB"); return item.Text; }).ToArray();
                timestamp = TimeStampLen; dropped = snapshot.Logs.Dropped;
            }
            if (timestamp < 0 || lines.Any(line => line.Length < timestamp)) throw new InvalidDataException("A log line is shorter than the timestamp length");
            var deadline = Stopwatch.StartNew();
            (long Milliseconds, int Matches) Measure(Regex regex)
            {
                var watch = Stopwatch.StartNew(); int matches = 0;
                foreach (var line in lines)
                {
                    if (deadline.Elapsed > TimeSpan.FromSeconds(2)) throw new TimeoutException("Benchmark exceeded two seconds");
                    if (regex.Match(line, timestamp).Success) matches++;
                }
                watch.Stop();
                if (deadline.Elapsed > TimeSpan.FromSeconds(2)) throw new TimeoutException("Benchmark exceeded two seconds");
                return (watch.ElapsedMilliseconds, matches);
            }
            var baseline = Measure(new Regex(".*?.*?", RegexOptions.Compiled, TimeSpan.FromMilliseconds(100)));
            var rows = new List<TriggerBenchmarkRow>();
            foreach (var item in selected)
            {
                var elapsed = Measure(item.Trigger.RegEx);
                rows.Add(new(item.Key, item.Trigger.Category, item.Trigger.RestrictToCategoryZone, elapsed.Milliseconds, elapsed.Matches));
            }
            lock (triggerGate)
            {
                CheckTriggerRevision(revision);
                if (selected.Any(item => !CustomTriggers.TryGetValue(item.Key, out var current) || !ReferenceEquals(current, item.Trigger)))
                    throw new InvalidOperationException("Trigger definitions changed during benchmark");
            }
            lock (CombatGate)
                if (actionSnapshot?.Revision != snapshotRevision || actionsClosed) throw new InvalidOperationException("Log snapshot changed during benchmark");
            return new(lines.Length, dropped, timestamp, baseline.Milliseconds, rows.ToArray());
        }
    }
}
