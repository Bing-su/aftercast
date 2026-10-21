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
