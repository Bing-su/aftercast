using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.PluginFixture;

// Exercise the real ACT member binding rather than injecting a Rust-only request.
public sealed class SpeechPlugin : IActPluginV1
{
    public void InitPlugin(TabPage tab, Label label) { ActGlobals.oFormActMain!.TTS("한글 <plain> & text"); label.Text = "Speech queued"; }
    public void DeInitPlugin() { }
}

public sealed class FailingSpeechPlugin : IActPluginV1
{
    public void InitPlugin(TabPage tab, Label label) { ActGlobals.oFormActMain!.TTS("fail"); }
    public void DeInitPlugin() { }
}

// A controlled ACT caller drives real timer start/warning/expiry APIs across the managed/Rust bridge.
public sealed class SoundPlugin : IActPluginV1
{
    public void InitPlugin(TabPage tab, Label label)
    {
        var root = ActGlobals.oFormActMain!;
        var wave = Path.Combine(root.AppDataFolder.FullName, "fixture.wav");
        // A quarter second of silent PCM verifies real file decoding without a loud fixture sound.
        using (var output = new BinaryWriter(File.Create(wave)))
        {
            output.Write("RIFF"u8); output.Write(36 + 12000); output.Write("WAVEfmt "u8); output.Write(16);
            output.Write((short)1); output.Write((short)1); output.Write(24000); output.Write(48000);
            output.Write((short)2); output.Write((short)16); output.Write("data"u8); output.Write(12000); output.Write(new byte[12000]);
        }
        root.PlaySound(wave);
        root.LastKnownTime = DateTime.UnixEpoch;
        var timers = ActGlobals.oFormSpellTimers!;
        timers.AddEditTimerDef(new TimerData("Sound fixture", "Audio")
        {
            TimerValue = 1, WarningValue = 1, StartSoundData = "tts ${attacker}", WarningSoundData = "beep"
        });
        timers.RebuildSpellTreeView();
        timers.NotifySpell("Caster", "Sound fixture", true, "Target", true);
        root.LastKnownTime = DateTime.UnixEpoch.AddSeconds(2);
        label.Text = "Sound and timer queued";
    }
    public void DeInitPlugin() { }
}

// No audio provider: observe the real TTS entry point through a temporary in-memory plugin delegate.
public sealed class SpeechRulesProbePlugin : IActPluginV1
{
    public void InitPlugin(TabPage tab, Label label)
    {
        var root = ActGlobals.oFormActMain!;
        var original = root.PlayTtsMethod;
        var originalSound = root.PlaySoundMethod;
        var speech = ""; var volumes = new List<int>();
        try
        {
            root.PlayTtsMethod = text => speech = text;
            root.PlaySoundMethod = (_, volume) => volumes.Add(volume);
            var wave = Path.Combine(root.AppDataFolder.FullName, "fake.wav");
            var sapi = Path.Combine(root.AppDataFolder.FullName, "sapi.wav");
            File.WriteAllText(wave, "synthetic"); File.WriteAllText(sapi, "synthetic");
            root.TTS("CAT"); root.PlaySound(wave); root.PlaySound(sapi);
            label.Text = speech + "|" + string.Join("|", volumes);
        }
        finally { root.PlayTtsMethod = original; root.PlaySoundMethod = originalSound; }
    }
    public void DeInitPlugin() { }
}

public sealed class MiscSoundProbePlugin : IActPluginV1
{
    public void InitPlugin(TabPage tab, Label label)
    {
        var root = ActGlobals.oFormActMain!;
        var previousSound = root.PlaySoundMethod; var previousTts = root.PlayTtsMethod;
        var result = "none";
        try
        {
            root.PlaySoundMethod = (source, volume) => result = $"sound:{source}:{volume}";
            root.PlayTtsMethod = text => result = "tts:" + text;
            root.PlayMiscSound();
            label.Text = result;
        }
        finally { root.PlaySoundMethod = previousSound; root.PlayTtsMethod = previousTts; }
    }
    public void DeInitPlugin() { }
}
