using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.PluginFixture;

// Synthetic zones intentionally share the same label to prove selection uses object identity.
public sealed class ZoneFixturePlugin : IActPluginV1
{
    private readonly ColumnsPlugin inner = new();
    private readonly List<ZoneData> zones = new();
    private FormActMain root = null!;
    public void InitPlugin(TabPage page, Label status)
    {
        inner.InitPlugin(page, status);
        root = ActGlobals.oFormActMain!;
        for (var group = 0; group < 2; group++)
        {
            var zone = new ZoneData(DateTime.UnixEpoch.AddHours(group), "Same Zone", false, false, false);
            zones.Add(zone);
            root.ZoneList.Add(zone);
            for (var index = 0; index < 2; index++)
            {
                var encounter = new EncounterData("Source", "Same Zone", zone) { Title = $"{group}-{index}" };
                zone.Items.Add(encounter);
                var time = DateTime.UnixEpoch.AddHours(group).AddMinutes(index);
                zone.AddCombatAction(new MasterSwing(1, false, "None", new Dnum(100), time, index, "Strike", "Source", "Physical", "Target"));
            }
        }
    }
    public void DeInitPlugin()
    {
        foreach (var zone in zones) root.ZoneList.Remove(zone);
        inner.DeInitPlugin();
    }
}
