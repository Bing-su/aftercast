using System.Drawing;
using System.Globalization;
using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Bxivact.PluginFixture;

// Exercise the Rust import boundary using an fixture plugin with deterministic actions.
public class ImportPlugin : IActPluginV1
{
    private FormActMain? root;
    private FormActMain.DateTimeLogParser? previousParser;
    public void InitPlugin(TabPage tab, Label status)
    {
        root = ActGlobals.oFormActMain!;
        previousParser = root.GetDateTimeFromLog;
        root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
        ActGlobals.charName = "Source";
        CombatantData.DamageTypeDataOutgoingDamage = "Damage";
        CombatantData.DamageTypeDataOutgoingHealing = "Healing";
        CombatantData.DamageTypeDataIncomingDamage = "Taken";
        CombatantData.DamageTypeDataIncomingHealing = "Received";
        CombatantData.OutgoingDamageTypeDataObjects = new()
        {
            ["Damage"] = new("Damage", -1, Color.Red),
            ["Healing"] = new("Healing", 1, Color.Green),
            ["All Out"] = new("All", 0, Color.Transparent)
        };
        CombatantData.IncomingDamageTypeDataObjects = new()
        {
            ["Taken"] = new("Taken", -1, Color.Red),
            ["Received"] = new("Received", 1, Color.Green),
            ["All In"] = new("All", 0, Color.Transparent)
        };
        CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = new() { [1] = new() { "Damage" } };
        CombatantData.SwingTypeToDamageTypeDataLinksIncoming = new() { [1] = new() { "Taken" } };
        root.BeforeLogLineRead += Read;
        status.Text = "Import fixture ready";
    }
    private void Read(bool import, LogLineEventArgs info)
    {
        if (!import) throw new InvalidOperationException("Fixture expects import delivery");
        var amount = long.Parse(info.logLine.Split('|')[1], CultureInfo.InvariantCulture);
        root!.SetEncounter(info.detectedTime, "Source", "Target");
        root.AddCombatAction(new(1, false, "None", new Dnum(amount), info.detectedTime,
            root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
    }
    public void DeInitPlugin()
    {
        root!.BeforeLogLineRead -= Read;
        root.GetDateTimeFromLog = previousParser!;
    }
}
