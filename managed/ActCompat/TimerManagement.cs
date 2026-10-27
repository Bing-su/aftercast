using Aftercast.ActCompat;

namespace Advanced_Combat_Tracker;

public partial class FormSpellTimers
{
    private ulong timerRevision = 1;
    private TimerItem[] lastTimerItems = [];
    private TimerItem[] TimerSnapshot()
    {
        var items = TimerDefs.Select(pair => new TimerItem(pair.Key,TimerDefinition.FromData(pair.Value))).ToArray();
        if (!items.SequenceEqual(lastTimerItems)) { timerRevision++; lastTimerItems = items; }
        return items;
    }
    private void CheckTimerRevision(ulong revision)
    {
        _ = TimerSnapshot();
        if (revision != timerRevision) throw new InvalidOperationException("Timer definitions changed; refresh before saving");
    }
    private static void ValidateTimerText(TimerDefinition definition)
    {
        ArgumentNullException.ThrowIfNull(definition);
        foreach (var value in new[] { definition.Name, definition.Category, definition.Tooltip, definition.StartSoundData, definition.WarningSoundData })
        {
            ArgumentNullException.ThrowIfNull(value);
            if (value.Length > 4096 || value.Contains('\0')) throw new ArgumentException("Timer text exceeds limits or contains NUL");
        }
    }
    private static TimerData CompileTimer(TimerDefinition definition)
    {
        ValidateTimerText(definition);
        if (string.IsNullOrWhiteSpace(definition.Name) || definition.Name.Length > 512 || definition.Category.Length > 512)
            throw new ArgumentException("Timer name/category is empty or exceeds 512 characters");
        // ACT's editor ranges; raw plugin/XML values remain signed integers and are not silently clamped.
        if (definition.TimerValue is < 0 or > 3600 || definition.WarningValue is < 0 or > 100 || definition.RemoveValue is < -600 or > 0)
            throw new ArgumentException("Timer editor ranges: duration 0..3600, warning 0..100, remove -600..0");
        return definition.ToData();
    }
    public TimerPage ReadTimerDefinitions(int offset,int limit)
    {
        if (offset < 0 || limit is < 1 or > 16) throw new ArgumentOutOfRangeException(nameof(limit));
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(IsDisposed,this);
            var items = TimerSnapshot();
            return new(timerRevision,items.Length,frames.Count,items.Skip(offset).Take(limit).ToArray());
        }
    }
    public ulong SaveTimerDefinition(string path,ulong revision,string? previousKey,TimerDefinition definition)
    {
        var compiled = CompileTimer(definition);
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(IsDisposed,this);
            CheckTimerRevision(revision);
            if (previousKey is not null && !TimerDefs.ContainsKey(previousKey)) throw new InvalidOperationException("Original timer no longer exists");
            if (TimerDefs.ContainsKey(compiled.Key) && (previousKey is null || TimerDefs.Comparer.Compare(compiled.Key,previousKey) != 0)) throw new InvalidOperationException("Timer name/category already exists");
            var replacement = new SortedDictionary<string,TimerData>(TimerDefs,TimerDefs.Comparer);
            if (previousKey is not null) replacement.Remove(previousKey);
            replacement.Add(compiled.Key,compiled);
            WriteTimerFile(path,replacement.Values);
            if (previousKey is not null) TimerDefs.Remove(previousKey);
            TimerDefs.Add(compiled.Key,compiled);
            RebuildSpellTreeView(); _ = TimerSnapshot(); return timerRevision;
        }
    }
    public ulong DeleteTimerDefinition(string path,ulong revision,string key)
    {
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(IsDisposed,this);
            CheckTimerRevision(revision);
            if (!TimerDefs.ContainsKey(key)) throw new InvalidOperationException("Timer no longer exists");
            var replacement = new SortedDictionary<string,TimerData>(TimerDefs,TimerDefs.Comparer);
            replacement.Remove(key);
            WriteTimerFile(path,replacement.Values);
            TimerDefs.Remove(key); RebuildSpellTreeView(); _ = TimerSnapshot(); return timerRevision;
        }
    }
    public ulong ReloadTimerDefinitions(string path,ulong revision)
    {
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(IsDisposed,this);
            CheckTimerRevision(revision);
            var file = ReadTimerFile(path);
            PublishTimerFile(file); _ = TimerSnapshot(); return timerRevision;
        }
    }
}
