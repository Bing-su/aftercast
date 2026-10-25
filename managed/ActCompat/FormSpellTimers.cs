using System.Windows.Forms;
using System.Text.RegularExpressions;

namespace Advanced_Combat_Tracker;

public delegate void SpellTimerEventDelegate(TimerFrame timer);

public class FormSpellTimers : Form
{
    private readonly Dictionary<string, TimerFrame> frames = new();
    private readonly object gate = new();
    public List<TimerMod> TimerMods { get; set; } = new();
    public SortedDictionary<string, List<string>> TimerLookups { get; } = new();
    public SortedDictionary<string, TimerData> TimerDefs { get; } = new();
    public Dictionary<string, string> WhiteList { get; set; } = new();
    public event SpellTimerEventDelegate? OnSpellTimerNotify;
    public event SpellTimerEventDelegate? OnSpellTimerWarning;
    public event SpellTimerEventDelegate? OnSpellTimerExpire;
    public event SpellTimerEventDelegate? OnSpellTimerRemoved;
    public void AddEditTimerDef(TimerData definition)
    {
        lock (gate) TimerDefs[definition.Key] = definition;
    }
    public void RemoveTimerDef(TimerData definition) { lock (gate) TimerDefs.Remove(definition.Key); }
    public void ClearTimerDefs() { lock (gate) TimerDefs.Clear(); }
    public void RebuildSpellTreeView()
    {
        lock (gate)
        {
            TimerLookups.Clear();
            foreach (var entry in TimerDefs)
            {
                if (!TimerLookups.TryGetValue(entry.Value.Name, out var keys)) TimerLookups.Add(entry.Value.Name, keys = new());
                keys.Add(entry.Key);
            }
        }
    }
    public List<TimerFrame> GetTimerFrames() => GetTimerFrames(0);
    public List<TimerFrame> GetTimerFrames(int panel)
    {
        lock (gate) return frames.Values.Where(frame => panel == 0 || panel == 1 && frame.TimerData.Panel1Display ||
            panel == 2 && frame.TimerData.Panel2Display).ToList();
    }
    public List<TimerMod> GetRecastMods(string caster)
    {
        lock (gate)
        {
            var name = caster.ToLower();
            return TimerMods.Where(mod => mod.Victim.ToLower() == name).ToList();
        }
    }
    public void ApplyTimerMod(string attacker, string victim, string name, float value, int duration)
    {
        lock (gate)
        {
            var mod = new TimerMod(attacker, victim, name, value, ActGlobals.oFormActMain!.LastEstimatedTime, TimeSpan.FromSeconds(duration));
            var index = TimerMods.IndexOf(mod);
            if (index < 0) TimerMods.Add(mod); else TimerMods[index] = mod;
        }
    }
    public void RemoveTimerMods(string owner)
    {
        lock (gate)
        {
            var name = owner.ToLower();
            TimerMods.RemoveAll(mod => mod.Attacker == name);
            foreach (var frame in frames.Values) frame.RemoveModOwner(owner, ActGlobals.oFormActMain!.LastKnownTime);
        }
    }
    public void DispellTimerMods(string target)
    {
        lock (gate)
        {
            var name = target.ToLower();
            TimerMods.RemoveAll(mod => mod.Victim == name);
            foreach (var frame in frames.Values) frame.RemoveModTarget(target, ActGlobals.oFormActMain!.LastKnownTime);
        }
    }
    public void NotifySpell(string attacker, string spell, bool self, string victim, bool success) =>
        NotifySpell(attacker, spell, self, victim, success, new());
    public void NotifySpell(string attacker, string spell, bool self, string victim, bool success, Dictionary<string, string> extra)
    {
        TimerFrame frame;
        string? startSound = null;
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(IsDisposed, this);
            extra.TryAdd("attacker", attacker);
            extra.TryAdd("victim", victim);
            attacker = attacker.ToLower();
            victim = victim.ToLower();
            if (!TimerLookups.TryGetValue(spell, out var keys)) return;
            TimerData? general = null, restricted = null;
            for (var i = keys.Count - 1; i >= 0; i--)
            {
                var data = TimerDefs[keys[i]];
                if (!data.ActiveInList) continue;
                if (!data.RestrictToCategory) general = data;
                else
                {
                    var category = data.Category.ToLower();
                    if (category == attacker || category == victim ||
                        category == ActGlobals.oFormActMain!.CurrentZone.ToLower()) restricted = data;
                }
            }
            var definition = restricted ?? general;
            if (definition is null || definition.RestrictToMe && !self && !WhiteList.ContainsKey(attacker) && !WhiteList.ContainsKey(victim)) return;
            var key = TimerFrame.GetKey(definition.Name, attacker);
            if (!frames.TryGetValue(key, out frame!)) frames.Add(key, frame = new(attacker, definition));
            frame.TimerData = definition;
            if (frame.OneOnly && frame.GetLargestVal(false) > 0) return;
            var now = ActGlobals.oFormActMain!.LastEstimatedTime;
            var recent = now - frame.GetMostRecentTime(true);
            if (recent < TimeSpan.FromSeconds(2)) return;
            var master = recent >= TimeSpan.FromSeconds(12) || definition.OnlyMasterTicks;
            var timer = new SpellTimer(master, definition.TimerValue, definition.Modable ? GetRecastMods(attacker) : new(), now) { ExtraInfo = extra };
            if (master) { frame.ExpireSounded = false; frame.WarningSounded = false; startSound = frame.StartSoundData; }
            frame.SpellTimers.Add(timer);
        }
        if (startSound is not null) TimerSound(frame, 0, startSound, extra);
        Notify(OnSpellTimerNotify, frame, nameof(OnSpellTimerNotify));
    }

    internal void Advance()
    {
        if (IsDisposed) return;
        TimerFrame? notification = null;
        SpellTimerEventDelegate? handlers = null;
        var eventName = "";
        lock (gate)
        {
            var now = ActGlobals.oFormActMain!.LastEstimatedTime;
            TimerMods.RemoveAll(mod => now - mod.LastUse > mod.UseDuration);
            foreach (var frame in frames.Values)
            {
                if (!frame.WarningSounded && frame.GetLargestVal(false) <= frame.WarningValue)
                {
                    frame.WarningSounded = true;
                    notification = frame; handlers = OnSpellTimerWarning; eventName = nameof(OnSpellTimerWarning); break;
                }
                if (!frame.ExpireSounded && frame.GetLargestVal(false) <= 0)
                {
                    frame.ExpireSounded = true;
                    notification = frame; handlers = OnSpellTimerExpire; eventName = nameof(OnSpellTimerExpire); break;
                }
                frame.SpellTimers.RemoveAll(timer => timer.TimeLeft < frame.TimerData.RemoveValue);
            }
        }
        // Repeater can restart or cancel a frame here; invoke plugins outside the frame gate.
        if (notification is not null)
        {
            TimerSound(notification, eventName == nameof(OnSpellTimerWarning) ? 1 : 2,
                notification.WarningSoundData, notification.SpellTimers.FirstOrDefault()?.ExtraInfo);
            Notify(handlers, notification, eventName);
        }
        if (IsDisposed) return;
        TimerFrame? removed = null;
        lock (gate)
        {
            var entry = frames.FirstOrDefault(pair => pair.Value.SpellTimers.Count == 0 || !pair.Value.MasterExists);
            if (entry.Value is not null) { frames.Remove(entry.Key); removed = entry.Value; }
        }
        if (removed is not null) Notify(OnSpellTimerRemoved, removed, nameof(OnSpellTimerRemoved));
    }
    private static void Notify(SpellTimerEventDelegate? handlers, TimerFrame frame, string name)
    {
        foreach (var callback in Delegate.EnumerateInvocationList(handlers))
            try { callback(frame); }
            catch (Exception error) { ActGlobals.oFormActMain?.WriteExceptionLog(error, "FormSpellTimers->" + name); }
    }

    private static string Substitute(string text, Dictionary<string, string> extra)
    {
        // ACT substitutes nested capture labels; cap expansion so a plugin's ${cycle} cannot stall the owner thread.
        for (var i = 0; i < 32; i++)
        {
            var match = Regex.Match(text, @"\$\{([^}]+)\}");
            if (!match.Success) return text;
            var key = match.Groups[1].Value;
            text = text[..match.Index] + extra.GetValueOrDefault(key, key) + text[(match.Index + match.Length)..];
            if (text.Length > 4096) throw new ArgumentException("Timer sound substitution exceeds its bounded size");
        }
        throw new ArgumentException("Timer sound substitution is recursive or exceeds 32 captures");
    }

    private static void TimerSound(TimerFrame frame, int state, string data, Dictionary<string, string>? extra = null)
    {
        var root = ActGlobals.oFormActMain!;
        try
        {
            if (state != 2 && extra is not null) data = Substitute(data, extra);
            if (state == 2) data = root.TimerExpireSound;
            else if (string.IsNullOrWhiteSpace(data)) data = state == 0 ? root.TimerStartSound : root.TimerWarningSound;
            if (data == "tts") data = "tts " + frame.Name + (state == 0 ? " started" : state == 1 ? " warning" : " expired");
            if (data == "none" || string.IsNullOrWhiteSpace(data)) return;
            if (data == "beep") root.Beep();
            else if (data.StartsWith("tts ", StringComparison.Ordinal)) root.TTS(data[3..]);
            else if (File.Exists(data)) root.PlaySound(data);
            else throw new FileNotFoundException("Could not find WAV: " + data, data);
        }
        catch (Exception error) { root.WriteExceptionLog(error, "SpellTimers->PlaySound"); }
    }
}
