using Advanced_Combat_Tracker;
using System.Windows.Forms;

namespace Aftercast.PluginFixture;

// Synthetic parser used only with explicitly imported temporary test files.
public class VcrImportPlugin : RecastImportPlugin, IActPluginV1
{
    public new void InitPlugin(TabPage tab, Label status)
    {
        base.InitPlugin(tab, status);
        CombatantData.DamageTypeDataNonSkillDamage = "Damage";
    }
}

// Supports a temporary followed fixture file; no real game data is read.
public class VcrLivePlugin : ImportPlugin, IActPluginV1
{
    protected override bool Live => true;
    public new void InitPlugin(TabPage tab, Label status)
    {
        base.InitPlugin(tab, status);
        CombatantData.DamageTypeDataNonSkillDamage = "Damage";
        CombatantData.DamageSwingTypes.Add(1);
    }
}
