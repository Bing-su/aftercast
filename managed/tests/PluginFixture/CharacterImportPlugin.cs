using System.Globalization;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.Testing;
namespace Aftercast.PluginFixture;

// This parser accepts only synthetic records; marker files coordinate cancellation without game input.
public class CharacterImportPlugin : IActPluginV1
{
    private FormActMain root = null!;
    private string? previousCharacter;
    private bool gated;
    protected virtual string InitialCharacter => "Source";
    public void InitPlugin(TabPage tab, Label status)
    {
        root = ActGlobals.oFormActMain!; previousCharacter = ActGlobals.charName;
        CombatFixture.ConfigureCategories(); ActGlobals.charName = InitialCharacter;
        root.LastKnownTime = DateTime.UnixEpoch.AddDays(1);
        root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
        root.BeforeLogLineRead += Read;
    }
    private void Read(bool import, LogLineEventArgs info)
    {
        if (!import) throw new InvalidOperationException("Expected synthetic import");
        var parts = info.logLine.Split('|');
        var attacker = parts[1] == "SELF" ? ActGlobals.charName! : parts[1];
        var victim = parts[2] == "SELF" ? ActGlobals.charName! : parts[2];
        root.SetEncounter(info.detectedTime, attacker, victim);
        root.AddCombatAction(new(1, false, "", new Dnum(long.Parse(parts[3], CultureInfo.InvariantCulture)), info.detectedTime,
            root.GlobalTimeSorter, "Strike", attacker, "Physical", victim));
        if (!gated && parts.Length > 4 && parts[4] == "gate")
        {
            gated = true;
            File.WriteAllText(Path.Combine(root.AppDataFolder.FullName, "character.ready"), "ready");
            if (!SpinWait.SpinUntil(() => File.Exists(Path.Combine(root.AppDataFolder.FullName, "character.release")), TimeSpan.FromSeconds(10)))
                throw new TimeoutException("Synthetic character gate");
        }
    }
    public void DeInitPlugin() { root.BeforeLogLineRead -= Read; ActGlobals.charName = previousCharacter; }
}
public sealed class ShortCharacterImportPlugin : CharacterImportPlugin
{
    protected override string InitialCharacter => "Al";
}
