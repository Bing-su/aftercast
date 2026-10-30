using System.Text;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class SoundChecks
{
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-sounds-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            var timers = ActGlobals.oFormSpellTimers!;
            host.ConfigureSpeech(true); host.ConfigureSound(true);
            CheckDuplicatePolicy(host, root);
            host.ConfigureAudio(61, 37, ["none", "none", "tts expired"]);
            var wave = Path.Combine(directory.FullName, "한글 sound.wav"); File.WriteAllBytes(wave, [0]);
            root.PlaySound(wave);
            var sound = host.DrainSound();
            if (Encoding.UTF8.GetString(sound.Bytes) != wave || sound.Volume != 61) throw new Exception("Sound path/volume changed");
            Parallel.For(0, 32, i => root.PlaySoundMethod(wave + i, 61));
            try { root.PlaySound(wave); throw new Exception("Unbounded sound queue"); } catch (InvalidOperationException) { }
            host.ConfigureSound(true);
            if (host.DrainSound().Bytes.Length != 0) throw new Exception("Replacing sound player retained old audio");
            var epoch = DateTime.UnixEpoch;
            root.LastKnownTime = epoch;
            var data = new TimerData("Fixture", "Audio") { TimerValue = 30, WarningValue = 10, StartSoundData = "tts ${attacker} ${missing}", WarningSoundData = "beep" };
            timers.AddEditTimerDef(data); timers.RebuildSpellTreeView();
            SpellTimerEventDelegate onNotify = _ => { if (Encoding.UTF8.GetString(host.DrainSpeech()) != " Caster missing") throw new Exception("Start audio did not precede notify"); };
            timers.OnSpellTimerNotify += onNotify;
            timers.NotifySpell("Caster", "Fixture", true, "Target", true);
            timers.OnSpellTimerNotify -= onNotify;
            root.LastKnownTime = epoch.AddSeconds(3);
            // Nonmaster ticks notify plugins, but do not request another start cue.
            timers.NotifySpell("Caster", "Fixture", true, "Target", true);
            if (host.DrainSpeech().Length != 0) throw new Exception("Nonmaster tick sounded");
            data.StartSoundData = "none";
            root.LastKnownTime = epoch.AddSeconds(21); host.AdvanceTimers();
            if (Encoding.UTF8.GetString(host.DrainSound().Bytes) != "beep") throw new Exception("Timer warning did not request beep");
            host.AdvanceTimers();
            if (host.DrainSound().Bytes.Length != 0) throw new Exception("Warning sound repeated");
            root.LastKnownTime = epoch.AddSeconds(31); host.AdvanceTimers();
            var expiry = Encoding.UTF8.GetString(host.DrainSpeech());
            if (expiry != " expired") throw new Exception("Expiry did not use configured speech: '" + expiry + "'; diagnostics=" + string.Join(" | ", host.DiagnosticsSnapshot.Item2));
            host.AdvanceTimers();
            if (host.DrainSpeech().Length != 0) throw new Exception("Expiry speech repeated");
            root.PlaySound(wave); host.BeginClosing();
            if (host.DrainSound().Bytes.Length != 0) throw new Exception("Closing retained sound");
        }
        finally { directory.Delete(true); }
    }
    private static void CheckDuplicatePolicy(HostEnvironment host, FormActMain root)
    {
        var now = DateTime.UnixEpoch;
        var delays = new List<int>();
        var setClock = typeof(FormActMain).GetMethod("ConfigureAudioTimingForTests", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!;
        setClock.Invoke(root, [(Func<DateTime>)(() => now), (Action<int>)(ms => { delays.Add(ms); now = now.AddMilliseconds(ms); })]);
        root.TTS("same"); root.TTS("same");
        if (Encoding.UTF8.GetString(host.DrainSpeech()) != "same" || host.DrainSpeech().Length != 0 || !delays.SequenceEqual([10])) throw new Exception("TTS duplicate window changed");
        now = now.AddMilliseconds(10); root.TTS("same");
        if (Encoding.UTF8.GetString(host.DrainSpeech()) != "same") throw new Exception("TTS boundary rejected");
        root.TTS("Same");
        if (Encoding.UTF8.GetString(host.DrainSpeech()) != "Same" || delays[^1] != 10) throw new Exception("TTS case or delay changed");
        var original = root.PlayTtsMethod; var calls = 0;
        root.PlayTtsMethod = _ => calls++;
        root.TTS("plugin"); root.TTS("plugin"); root.PlayTtsMethod = original;
        if (calls != 2) throw new Exception("Plugin delegate incorrectly suppressed");
        root.PlaySoundMethod("same.wav", 10); root.PlaySoundMethod("same.wav", 90);
        if (Encoding.UTF8.GetString(host.DrainSound().Bytes) != "same.wav" || host.DrainSound().Bytes.Length != 0 || delays[^1] != 50) throw new Exception("Sound duplicate window changed");
        now = now.AddMilliseconds(50); root.PlaySoundMethod("same.wav", 90);
        if (host.DrainSound().Volume != 90) throw new Exception("Sound boundary rejected");
        root.PlaySoundMethod("other.wav", 30);
        if (Encoding.UTF8.GetString(host.DrainSound().Bytes) != "other.wav" || delays[^1] != 50) throw new Exception("Sound different-key delay changed");
        host.ConfigureAudio(61, 37, ["none", "none", "none"], "beep", "");
        root.PlayMiscSound(); root.PlayMiscSound();
        if (Encoding.UTF8.GetString(host.DrainSound().Bytes) != "beep" || Encoding.UTF8.GetString(host.DrainSound().Bytes) != "beep") throw new Exception("Beep incorrectly suppressed");
        host.ConfigureAudio(61, 37, ["none", "none", "tts expired"]);
    }

}
