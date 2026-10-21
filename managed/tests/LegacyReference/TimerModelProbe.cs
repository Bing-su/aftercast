using System.Diagnostics;
using System.Reflection;
using System.Runtime.CompilerServices;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class TimerModelProbe
{
    public static string Run()
    {
        var previous = ActGlobals.oFormActMain;
        var main = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        GC.SuppressFinalize(main);
        var epoch = DateTime.UnixEpoch;
        var flags = BindingFlags.Instance | BindingFlags.NonPublic;
        // Freeze the original estimate and establish the replacement's monotonic baseline without creating windows.
        typeof(FormActMain).GetField("lastKnownTime", flags)!.SetValue(main, epoch.AddSeconds(100));
        typeof(FormActMain).GetField("estimatedTimer", flags)?.SetValue(main, new Stopwatch());
        typeof(FormActMain).GetField("logGate", flags)?.SetValue(main, new object());
        typeof(FormActMain).GetField("lastKnownStamp", flags)?.SetValue(main, Stopwatch.GetTimestamp());
        void Check(bool valid) { if (!valid) throw new Exception("Original-reference timer model mismatch"); }
        try
        {
            ActGlobals.oFormActMain = main;
            var data = new TimerData("FiXtUrE", "");
            Check(data.Category == " General" && data.Key == " general|fixture" && data.TimerValue == 30 &&
                data.RemoveValue == -15 && data.WarningValue == 10 && data.Panel1Display && !data.Panel2Display && data.Modable);
            var frame = new TimerFrame("Caster", data);
            Check(frame.GetLargestVal(false) == 0 && frame.GetSmallestVal(false) == int.MaxValue &&
                frame.GetMostRecentTime(false) == DateTime.MinValue && !frame.MasterExists);
            var mods = new List<TimerMod>
            {
                new("owner", "target", "A", .5f, epoch, TimeSpan.FromSeconds(30)),
                new("other", "target", "A", 1f, epoch, TimeSpan.FromSeconds(30)),
                new("third", "target", "B", -.25f, epoch, TimeSpan.FromSeconds(30))
            };
            var timer = new SpellTimer(true, 30, mods, epoch.AddSeconds(80));
            frame.SpellTimers.Add(timer);
            frame.SpellTimers.Add(new(false, 60, new(), epoch.AddSeconds(90)));
            Check(timer.TimerModValue == .25f && timer.TimerFinalDuration == 37 && timer.TimeLeft == 17);
            Check(frame.GetLargestVal(false) == 17 && frame.GetLargestVal(true) == 50 && frame.GetSmallestVal(true) == 17 &&
                frame.TimerVals.SequenceEqual(new[] { 17, 50 }) && frame.TopModAmount == .25f && frame.MasterExists);
            Check(frame.GetMostRecentTime(false) == epoch.AddSeconds(80) && frame.GetMostRecentTime(true) == epoch.AddSeconds(90));
            timer.RemoveModOwner("OWNER", epoch.AddSeconds(82));
            Check(mods.Count == 3);
            timer.RemoveModOwner("OWNER", epoch.AddSeconds(81));
            Check(mods.Count == 2 && timer.TimerFinalDuration == 52);
            timer.RemoveModTarget("TARGET", epoch.AddSeconds(80.5));
            Check(mods.Count == 0 && timer.TimerFinalDuration == 30);
            Check(frame.ToString() == "FiXtUrE - Caster" && data.Equals(new TimerData("fixture", " general")));
            return "Timer models";
        }
        finally { ActGlobals.oFormActMain = previous; }
    }
}
