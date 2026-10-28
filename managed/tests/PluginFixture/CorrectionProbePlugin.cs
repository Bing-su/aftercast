using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.PluginFixture;

public sealed class CorrectionProbePlugin : IActPluginV1
{
    private readonly ImportPlugin parser = new();
    private readonly List<string> observations = new();
    private FormActMain root = null!;
    private List<int> previousDamageTypes = null!;
    public void InitPlugin(TabPage page, Label status)
    {
        parser.InitPlugin(page, status); root = ActGlobals.oFormActMain!;
        previousDamageTypes = CombatantData.DamageSwingTypes; CombatantData.DamageSwingTypes = [1];
        root.BeforeCombatAction += Observe;
        EncounterData.ColumnDefs["correction-probe"] = new("Probe", false, "TEXT", "probe",
            _ => string.Join(",", observations), _ => "probe");
    }
    private void Observe(bool import, CombatActionEventArgs action)
    {
        if (!import) throw new InvalidOperationException("Synthetic import expected");
        observations.Add(action.attacker + ":" + action.victim + ":" + action.combatAction.Attacker);
    }
    public void DeInitPlugin()
    {
        root.BeforeCombatAction -= Observe; EncounterData.ColumnDefs.Remove("correction-probe");
        CombatantData.DamageSwingTypes = previousDamageTypes; parser.DeInitPlugin();
    }
}
