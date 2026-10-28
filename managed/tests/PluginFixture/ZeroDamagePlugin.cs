using System.Globalization;
using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.PluginFixture;

// Synthetic callback observes the setting and original action values before aggregation.
public sealed class ZeroDamagePlugin : IActPluginV1
{
    private readonly ImportPlugin parser = new();
    private readonly List<string> observations = new();
    private FormActMain root = null!;
    public void InitPlugin(TabPage page, Label status)
    {
        parser.InitPlugin(page, status);
        root = ActGlobals.oFormActMain!;
        status.Text = ActGlobals.blockIsHit ? "zero-hit:on" : "zero-hit:off";
        root.BeforeCombatAction += Observe;
        EncounterData.ColumnDefs["zero-hit-probe"] = new("Probe", false, "TEXT", "probe",
            _ => string.Join(",", observations), _ => "probe");
    }
    private void Observe(bool import, CombatActionEventArgs action)
    {
        if (!import || action.combatAction.Damage.Number != action.damage.Number)
            throw new InvalidOperationException("Synthetic input changed before plugin callback");
        observations.Add(ActGlobals.blockIsHit + ":" + action.damage.Number.ToString(CultureInfo.InvariantCulture));
    }
    public void DeInitPlugin()
    {
        root.BeforeCombatAction -= Observe;
        EncounterData.ColumnDefs.Remove("zero-hit-probe");
        parser.DeInitPlugin();
    }
}
