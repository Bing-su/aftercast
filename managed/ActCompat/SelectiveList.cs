namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    public void SelectiveListAdd(string player) { lock (ActGlobals.actionDataLock) ActGlobals.selectiveList[player.ToUpper()] = true; }
    public void SelectiveListRemove(string player, bool removeEntry)
    {
        lock (ActGlobals.actionDataLock)
        {
            var key = player.ToUpper();
            if (!ActGlobals.selectiveList.ContainsKey(key)) return;
            if (removeEntry) ActGlobals.selectiveList.Remove(key); else ActGlobals.selectiveList[key] = false;
        }
    }
    public void SelectiveListClear() { lock (ActGlobals.actionDataLock) ActGlobals.selectiveList.Clear(); }
    public void SelectiveListUncheckAll()
    {
        lock (ActGlobals.actionDataLock) foreach (var key in ActGlobals.selectiveList.Keys.ToArray()) ActGlobals.selectiveList[key] = false;
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
