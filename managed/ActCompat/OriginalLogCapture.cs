namespace Aftercast.ActCompat
{
    // Immutable transport evidence: this is the original dispatched text, not plugin acceptance or durable storage.
    public sealed record OriginalLogRecord(bool IsImport, string Path, string Line, DateTime Time, int TimeSorter);
}

namespace Advanced_Combat_Tracker
{
    public partial class FormActMain
    {
        internal event Action<Aftercast.ActCompat.OriginalLogRecord>? OriginalLogRecordRead;

        private void PublishOriginalLogRecord(bool isImport, string path, string line, DateTime time, int timeSorter)
        {
            var observers = OriginalLogRecordRead;
            if (observers is null || line.Length == 0) return;
            // One immutable snapshot is shared by all observers, e.g. later hook edits cannot replace its Line.
            var record = new Aftercast.ActCompat.OriginalLogRecord(isImport, path, line, time, timeSorter);
            foreach (var observer in Delegate.EnumerateInvocationList(observers))
                try { observer(record); }
                catch (Exception error)
                {
                    // Optional capture failures remain visible without changing import/parser success.
                    WriteExceptionLog(error, "Original log record observer");
                }
        }
    }
}
