namespace Advanced_Combat_Tracker;

public class TimerMod(string attacker, string victim, string modName, float modValue, DateTime lastUse,
    TimeSpan useDuration) : IEquatable<TimerMod>
{
    public string Attacker { get; set; } = attacker;
    public string Victim { get; set; } = victim;
    public string ModName { get; set; } = modName;
    public float ModValue { get; set; } = modValue;
    public DateTime LastUse { get; set; } = lastUse;
    public TimeSpan UseDuration { get; set; } = useDuration;
    public override string ToString() => $"{Attacker}>{Victim}({ModName})";
    public bool Equals(TimerMod? other) => other!.ToString().Equals(ToString());
}

public class SpellTimer(bool masterTimer, int timerDuration, List<TimerMod> timerMods, DateTime timerStart)
{
    public Dictionary<string, string> ExtraInfo = new();
    public bool MasterTimer { get; } = masterTimer;
    public DateTime StartTime { get; } = timerStart;
    public float TimerModValue
    {
        get
        {
            var names = new HashSet<string>();
            var total = 0f;
            foreach (var mod in timerMods)
                if (names.Add(mod.ModName)) total += mod.ModValue;
            return total;
        }
    }
    public int TimerFinalDuration => (int)(timerDuration * (1f + TimerModValue));
    // ACT rounds elapsed seconds to even, e.g. 2.5 seconds consumes two rather than three seconds.
    public int TimeLeft => TimerFinalDuration - Convert.ToInt32((ActGlobals.oFormActMain!.LastEstimatedTime - StartTime).TotalSeconds);
    public void RemoveModOwner(string owner, DateTime deathTime) =>
        timerMods.RemoveAll(mod => mod.Attacker == owner.ToLower() && deathTime - StartTime < TimeSpan.FromSeconds(2));
    public void RemoveModTarget(string target, DateTime dispellTime) =>
        timerMods.RemoveAll(mod => mod.Victim == target.ToLower() && dispellTime - StartTime < TimeSpan.FromSeconds(1));
}
