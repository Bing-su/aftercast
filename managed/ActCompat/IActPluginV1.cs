using System.Windows.Forms;

namespace Advanced_Combat_Tracker;

// Preserve original CLI signatures for existing plugins.
public interface IActPluginV1
{
    void InitPlugin(TabPage pluginScreenSpace, Label pluginStatusText);
    void DeInitPlugin();
}
