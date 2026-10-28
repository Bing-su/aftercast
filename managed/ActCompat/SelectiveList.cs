namespace Advanced_Combat_Tracker;

// Values and names match ACT's public plugin contract.
public enum SelectiveParsingModeEnum
{
    None, SelectiveGraphsExports, SelectiveGraphsExportsAndAlliedTotals,
    SelectiveEncounters, SelectiveEncountersStrict
}

public partial class FormActMain
{
    private SelectiveParsingModeEnum selectiveMode;
    private long selectiveRevision;
    internal bool FilterExports => selectiveMode is SelectiveParsingModeEnum.SelectiveGraphsExports or SelectiveParsingModeEnum.SelectiveGraphsExportsAndAlliedTotals;
    internal bool FilterAlliedTotals => selectiveMode == SelectiveParsingModeEnum.SelectiveGraphsExportsAndAlliedTotals;
    public SelectiveParsingModeEnum SelectiveListGetMode() { lock (ActGlobals.actionDataLock) return selectiveMode; }
    internal bool RecordSelective => selectiveMode is SelectiveParsingModeEnum.SelectiveEncounters or SelectiveParsingModeEnum.SelectiveEncountersStrict;
    internal bool RecordStrict => selectiveMode == SelectiveParsingModeEnum.SelectiveEncountersStrict;
    internal static SortedDictionary<string, bool> ValidateSelectiveParsing(SelectiveParsingModeEnum mode, string[] selected)
    {
        ArgumentNullException.ThrowIfNull(selected);
        if (!Enum.IsDefined(mode) || selected.Length > 4096) throw new ArgumentException("Invalid selective parsing options");
        var list = new SortedDictionary<string, bool>();
        foreach (var name in selected)
        {
            if (string.IsNullOrWhiteSpace(name) || name.Contains('\0') || System.Text.Encoding.UTF8.GetByteCount(name) > 1024)
                throw new ArgumentException("Invalid selected combatant name");
            if (!list.TryAdd(name.ToUpper(), true)) throw new ArgumentException("Duplicate selected combatant name");
        }
        return list;
    }
    internal void ConfigureSelectiveParsing(SelectiveParsingModeEnum mode, string[] selected)
    {
        var list = ValidateSelectiveParsing(mode, selected);
        lock (ActGlobals.actionDataLock) { selectiveMode = mode; ActGlobals.selectiveList = list; selectiveRevision++; }
    }

    public void SelectiveListAdd(string player) { lock (ActGlobals.actionDataLock) { ActGlobals.selectiveList[player.ToUpper()] = true; selectiveRevision++; } }
    public void SelectiveListRemove(string player, bool removeEntry)
    {
        lock (ActGlobals.actionDataLock)
        {
            var key = player.ToUpper();
            if (!ActGlobals.selectiveList.ContainsKey(key)) return;
            if (removeEntry) ActGlobals.selectiveList.Remove(key); else ActGlobals.selectiveList[key] = false;
            selectiveRevision++;
        }
    }
    public void SelectiveListClear() { lock (ActGlobals.actionDataLock) { ActGlobals.selectiveList.Clear(); selectiveRevision++; } }
    public void SelectiveListUncheckAll()
    {
        lock (ActGlobals.actionDataLock) { foreach (var key in ActGlobals.selectiveList.Keys.ToArray()) ActGlobals.selectiveList[key] = false; selectiveRevision++; }
    }
    public bool SelectiveListGetSelected(string player)
    {
        lock (ActGlobals.actionDataLock) return ActGlobals.selectiveList.TryGetValue(player.ToUpper(), out var selected) && selected;
    }
    public void PlayMiscSound() => throw new NotSupportedException("ACT sound configuration/playback is not implemented");
}

public static partial class ActGlobals
{
    internal static SortedDictionary<string, bool> selectiveList = new();
}
