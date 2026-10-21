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
