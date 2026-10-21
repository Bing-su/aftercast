namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private readonly Dictionary<string, bool> selectivePlayers = new();
    // Selection registration is available independently of the still-unsupported selective combat filtering policy.
    public void SelectiveListAdd(string player) { lock (CombatGate) selectivePlayers[player.ToUpper()] = true; }
    public void SelectiveListRemove(string player, bool removeEntry)
    {
        lock (CombatGate)
        {
            var key = player.ToUpper();
            if (!selectivePlayers.ContainsKey(key)) return;
            if (removeEntry) selectivePlayers.Remove(key); else selectivePlayers[key] = false;
        }
    }
    public void SelectiveListClear() { lock (CombatGate) selectivePlayers.Clear(); }
    public void SelectiveListUncheckAll()
    {
        lock (CombatGate) foreach (var key in selectivePlayers.Keys.ToArray()) selectivePlayers[key] = false;
    }
    public bool SelectiveListGetSelected(string player)
    {
        lock (CombatGate) return selectivePlayers.TryGetValue(player.ToUpper(), out var selected) && selected;
    }
    public void PlayMiscSound() => throw new NotSupportedException("ACT sound configuration/playback is not implemented");
}
