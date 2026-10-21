namespace Advanced_Combat_Tracker;

public class TimerFrame(string combatant, TimerData timerData) : IEquatable<TimerFrame>, IComparable<TimerFrame>
{
    public bool WarningSounded { get; set; }
    public bool ExpireSounded { get; set; }
    public TimerData TimerData { get; set; } = timerData;
    public string Combatant { get; set; } = combatant;
    public List<SpellTimer> SpellTimers { get; set; } = new();
    public string Name => TimerData.Name;
    public bool RestrictToMe => TimerData.RestrictToMe;
    public bool OneOnly => TimerData.AbsoluteTiming;
    public string StartSoundData => TimerData.StartSoundData;
    public string WarningSoundData => TimerData.WarningSoundData;
    public int WarningValue => TimerData.WarningValue;
    public bool RadialDisplay => TimerData.RadialDisplay;
    public int[] TimerVals => SpellTimers.Select(timer => timer.TimeLeft).Order().ToArray();
    public float TopModAmount
    {
        get
        {
            for (var i = SpellTimers.Count - 1; i >= 0; i--)
                if (SpellTimers.Count > i && SpellTimers[i] is { MasterTimer: true } timer) return timer.TimerModValue;
            return 0;
        }
    }
    public bool MasterExists
    {
        get
        {
            for (var i = 0; i < SpellTimers.Count; i++)
                if (SpellTimers.Count > i && SpellTimers[i] is { MasterTimer: true }) return true;
            return false;
        }
    }
    public int GetLargestVal(bool includeNonMaster)
    {
        var largest = int.MinValue;
        // The parser can append ticks while an owner-thread plugin reads the live frame.
        for (var i = 0; i < SpellTimers.Count; i++)
            if (SpellTimers.Count > i && SpellTimers[i] is { } timer && (timer.MasterTimer || includeNonMaster))
                largest = Math.Max(largest, timer.TimeLeft);
        return largest == int.MinValue ? 0 : largest;
    }
    public int GetSmallestVal(bool includeNonMaster)
    {
        var smallest = int.MaxValue;
        foreach (var timer in SpellTimers)
            if (timer.MasterTimer || includeNonMaster) smallest = Math.Min(smallest, timer.TimeLeft);
        // Preserve ACT's empty-list sentinel rather than normalizing it to GetLargestVal's zero.
        return smallest == int.MinValue ? 0 : smallest;
    }
    public DateTime GetMostRecentTime(bool includeNonMaster)
    {
        var latest = DateTime.MinValue;
        for (var i = 0; i < SpellTimers.Count; i++)
            if (SpellTimers.Count > i && SpellTimers[i] is { } timer && (timer.MasterTimer || includeNonMaster) && timer.StartTime > latest)
                latest = timer.StartTime;
        return latest;
    }
    public void RemoveModOwner(string owner, DateTime deathTime)
    {
        for (var i = 0; i < SpellTimers.Count; i++)
            if (SpellTimers.Count > i && SpellTimers[i] is { MasterTimer: true } timer) timer.RemoveModOwner(owner, deathTime);
    }
    public void RemoveModTarget(string target, DateTime dispellTime)
    {
        for (var i = 0; i < SpellTimers.Count; i++)
            if (SpellTimers.Count > i && SpellTimers[i] is { MasterTimer: true } timer) timer.RemoveModTarget(target, dispellTime);
    }
    public static string GetKey(string spellName, string combatant) => $"{spellName} - {combatant}";
    public override string ToString() => GetKey(Name, Combatant);
    public bool Equals(TimerFrame? other) => ToString().Equals(other!.ToString());
    public int CompareTo(TimerFrame? other)
    {
        var result = Combatant.CompareTo(other!.Combatant);
        return result != 0 ? result : GetMostRecentTime(false).CompareTo(other.GetMostRecentTime(false));
    }
}
