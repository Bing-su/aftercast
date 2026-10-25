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
            host.ConfigureAudio(61, 37, ["none", "none", "tts expired"]);
            var wave = Path.Combine(directory.FullName, "한글 sound.wav"); File.WriteAllBytes(wave, [0]);
            root.PlaySound(wave);
            var sound = host.DrainSound();
            if (Encoding.UTF8.GetString(sound.Bytes) != wave || sound.Volume != 61) throw new Exception("Sound path/volume changed");
            Parallel.For(0, 32, _ => root.PlaySound(wave));
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
}
