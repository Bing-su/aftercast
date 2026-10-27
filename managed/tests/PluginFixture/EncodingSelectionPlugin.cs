using Advanced_Combat_Tracker;
using System.Text;
using System.Windows.Forms;

namespace Aftercast.PluginFixture;

// Select only synthetic import decoding; no files, capture or external service is used.
public sealed class KoreanEncodingPlugin : IActPluginV1
{
    private readonly ImportPlugin parser = new();
    private FormActMain? root;
    private Encoding? previous;
    public void InitPlugin(TabPage tab, Label status)
    {
        root = ActGlobals.oFormActMain!;
        previous = root.LogEncoding;
        root.LogEncoding = Encoding.GetEncoding(949, EncoderFallback.ExceptionFallback, DecoderFallback.ExceptionFallback);
        parser.InitPlugin(tab,status);
    }
    public void DeInitPlugin()
    {
        parser.DeInitPlugin();
        root!.LogEncoding = previous!;
    }
}
