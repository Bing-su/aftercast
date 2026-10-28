using System.Drawing;
using System.Globalization;
using Aftercast.ActCompat;

namespace Aftercast.ActCompat
{
    public sealed record TimerSnippetPreview(ulong Revision, string Key, bool Exists, TimerDefinition Definition);
}

namespace Advanced_Combat_Tracker
{
    public partial class FormSpellTimers
    {
        public TimerSnippetPreview PreviewTimerSnippet(string snippet)
        {
            var element = SnippetXml.Read(snippet, "Spell", "N", "C", "T", "R", "OM", "A", "WV", "RD", "M", "Tt", "SS", "WS", "FC", "RV", "RC");
            if (!element.HasAttribute("N") || !element.HasAttribute("C")) throw new InvalidDataException("Spell snippet requires N and C attributes");
            var timer = new TimerData(element.GetAttribute("N"), element.GetAttribute("C"));
            int Number(string name, int fallback) => int.TryParse(element.GetAttribute(name), NumberStyles.Integer, CultureInfo.CurrentCulture, out var parsed) ? parsed : fallback;
            bool Flag(string name, bool fallback) => element.HasAttribute(name) ? element.GetAttribute(name) == "T" : fallback;
            timer.TimerValue = Number("T", timer.TimerValue);
            timer.RestrictToMe = Flag("R", timer.RestrictToMe);
            timer.OnlyMasterTicks = Flag("OM", timer.OnlyMasterTicks);
            timer.AbsoluteTiming = Flag("A", timer.AbsoluteTiming);
            timer.WarningValue = Number("WV", timer.WarningValue);
            timer.RadialDisplay = Flag("RD", timer.RadialDisplay);
            timer.Modable = Flag("M", timer.Modable);
            if (element.HasAttribute("Tt")) timer.Tooltip = element.GetAttribute("Tt");
            if (element.HasAttribute("SS")) timer.StartSoundData = element.GetAttribute("SS");
            if (element.HasAttribute("WS")) timer.WarningSoundData = element.GetAttribute("WS");
            timer.FillColor = Color.FromArgb(Number("FC", timer.FillColor.ToArgb()));
            timer.RemoveValue = Number("RV", timer.RemoveValue);
            timer.RestrictToCategory = Flag("RC", timer.RestrictToCategory);
            var definition = TimerDefinition.FromData(timer);
            _ = CompileTimer(definition); // Use the same explicit editor ranges before showing a savable item.
            lock (gate)
            {
                ObjectDisposedException.ThrowIf(IsDisposed, this);
                _ = TimerSnapshot();
                return new(timerRevision, timer.Key, TimerDefs.ContainsKey(timer.Key), definition);
            }
        }
    }
}
