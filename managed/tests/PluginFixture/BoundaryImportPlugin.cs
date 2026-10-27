using System.Globalization;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.Testing;
namespace Aftercast.PluginFixture;

// Synthetic boundary markers exercise policy through the actual isolated worker, without game capture.
public sealed class BoundaryImportPlugin : IActPluginV1
{
    private FormActMain root=null!;
    private FormActMain.DateTimeLogParser previous=null!;
    private bool gated;
    public void InitPlugin(TabPage tab,Label status)
    {
        root=ActGlobals.oFormActMain!;previous=root.GetDateTimeFromLog;root.TimeStampLen=0;
        root.GetDateTimeFromLog=line=>line.StartsWith("U|",StringComparison.Ordinal)?DateTime.MinValue:DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0],CultureInfo.InvariantCulture));
        CombatFixture.ConfigureCategories();root.BeforeLogLineRead+=Read;status.Text="Synthetic import boundary fixture";
    }
    private void Read(bool import,LogLineEventArgs info)
    {
        if(!import)throw new InvalidOperationException("Synthetic fixture requires import");
        var parts=info.originalLogLine.Split('|');
        if(parts[1]=="E"){root.EndCombat(false);return;}
        if(parts[1]!="A")return;
        root.SetEncounter(info.detectedTime,"Source","Target");
        root.AddCombatAction(new(1,false,"",new Dnum(long.Parse(parts[2],CultureInfo.InvariantCulture)),info.detectedTime,root.GlobalTimeSorter,"Strike","Source","Physical","Target"));
        if(!gated&&root.LogFilePath.EndsWith("cancel.log",StringComparison.Ordinal))
        {
            gated=true;var ready=Path.Combine(root.AppDataFolder.FullName,"boundary.ready");var release=Path.Combine(root.AppDataFolder.FullName,"boundary.release");
            File.WriteAllText(ready,"synthetic callback gate");
            if(!SpinWait.SpinUntil(()=>File.Exists(release),TimeSpan.FromSeconds(10)))throw new TimeoutException("Synthetic boundary gate was not released");
        }
    }
    public void DeInitPlugin(){root.BeforeLogLineRead-=Read;root.GetDateTimeFromLog=previous;}
}
