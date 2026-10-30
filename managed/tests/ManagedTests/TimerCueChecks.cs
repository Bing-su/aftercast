using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class TimerCueChecks
{
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-timer-cues-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            var timers = ActGlobals.oFormSpellTimers!;
            var sound = new List<string>(); var speech = new List<string>();
            root.PlaySoundMethod = (value, _) => sound.Add(value);
            root.PlayTtsMethod = value => speech.Add(value);
            host.ConfigureAudio(100, 100, ["beep", "tts", "beep"]);
            var epoch = DateTime.UnixEpoch;
            var first = new TimerData("First", "Audio") { TimerValue = 30, WarningValue = 10 };
            timers.AddEditTimerDef(first); timers.RebuildSpellTreeView();
            root.LastKnownTime = epoch; timers.NotifySpell("Caster", "First", true, "Target", true);
            if (!sound.SequenceEqual(["beep"])) throw new Exception("Common start cue missing");
            root.LastKnownTime = epoch.AddSeconds(21); host.AdvanceTimers();
            if (!speech.SequenceEqual([" First warning"])) throw new Exception("Common warning cue missing: " + string.Join(" | ", speech) + " diagnostics: " + string.Join(" | ", host.DiagnosticsSnapshot.Item2));
            root.LastKnownTime = epoch.AddSeconds(31); host.AdvanceTimers();
            if (!sound.SequenceEqual(["beep", "beep"])) throw new Exception("Common expiry cue missing");
            sound.Clear(); speech.Clear();
            var second = new TimerData("Second", "Audio") { TimerValue = 30, WarningValue = 10, StartSoundData = "none", WarningSoundData = "beep" };
            timers.AddEditTimerDef(second); timers.RebuildSpellTreeView();
            root.LastKnownTime = epoch.AddSeconds(40); timers.NotifySpell("Caster", "Second", true, "Target", true);
            if (sound.Count != 0 || speech.Count != 0) throw new Exception("Per-timer none did not override start");
            root.LastKnownTime = epoch.AddSeconds(61); host.AdvanceTimers();
            if (!sound.SequenceEqual(["beep"]) || speech.Count != 0) throw new Exception("Per-timer warning did not override common setting");
            root.LastKnownTime = epoch.AddSeconds(71); host.AdvanceTimers();
            if (!sound.SequenceEqual(["beep", "beep"])) throw new Exception("Expiry must use common cue");
            sound.Clear(); speech.Clear();
            host.ConfigureAudio(100, 100, ["none", "none", "none"]);
            var muted = new TimerData("Muted", "Audio") { TimerValue = 30, WarningValue = 10 };
            timers.AddEditTimerDef(muted); timers.RebuildSpellTreeView();
            root.LastKnownTime = epoch.AddSeconds(80); timers.NotifySpell("Caster", "Muted", true, "Target", true);
            root.LastKnownTime = epoch.AddSeconds(101); host.AdvanceTimers();
            if (sound.Count != 0 || speech.Count != 0) throw new Exception("Disabled cues emitted audio");
            var wave = Path.Combine(directory.FullName, "synthetic.wav");
            File.WriteAllBytes(wave, [0]);
            host.ConfigureAudio(100, 100, [wave, "none", "none"]);
            var fileCue = new TimerData("FileCue", "Audio");
            timers.AddEditTimerDef(fileCue); timers.RebuildSpellTreeView();
            root.LastKnownTime = epoch.AddSeconds(105); timers.NotifySpell("Caster", "FileCue", true, "Target", true);
            if (!sound.SequenceEqual([wave])) throw new Exception("Synthetic WAV path was not passed to sink");
            var beforeMissing = host.DiagnosticsSnapshot.Item1;
            host.ConfigureAudio(100, 100, [Path.Combine(directory.FullName, "missing.wav"), "none", "none"]);
            var missing = new TimerData("MissingCue", "Audio");
            timers.AddEditTimerDef(missing); timers.RebuildSpellTreeView();
            root.LastKnownTime = epoch.AddSeconds(106); timers.NotifySpell("Caster", "MissingCue", true, "Target", true);
            if (host.DiagnosticsSnapshot.Item1 <= beforeMissing) throw new Exception("Missing file was not diagnosed");
            var before = host.DiagnosticsSnapshot.Item1;
            root.PlaySoundMethod = (_, _) => throw new InvalidOperationException("synthetic sink failure");
            host.ConfigureAudio(100, 100, ["beep", "none", "none"]);
            var failing = new TimerData("Failing", "Audio");
            timers.AddEditTimerDef(failing); timers.RebuildSpellTreeView();
            root.LastKnownTime = epoch.AddSeconds(110); timers.NotifySpell("Caster", "Failing", true, "Target", true);
            if (host.DiagnosticsSnapshot.Item1 <= before) throw new Exception("Sink failure was not diagnosed");
            host.BeginClosing();
        }
        finally { directory.Delete(true); }
    }
}
