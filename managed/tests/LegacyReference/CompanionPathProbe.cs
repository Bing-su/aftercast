using System.Runtime.CompilerServices;
using System.Diagnostics;
using System.Reflection;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class CompanionPathProbe
{
    public static string Run()
    {
        var root = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        GC.SuppressFinalize(root);
        foreach (var (main, name) in new[] { (Path.Combine("logs", "Network_20261027.log"), "Overlay"),
            ("Network.log", "한글"), (Path.Combine("logs", "a.b.txt"), "events"), ("plain", "State") })
        {
            var expected = Path.Combine(Path.GetDirectoryName(main)!, "CompanionLogs", Path.GetFileNameWithoutExtension(main) + "." + name + Path.GetExtension(main));
            if (root.GetCompanionFilePath(main, name) != expected) throw new Exception("Companion path contract changed");
        }
        foreach (var name in new[] { "", " ", "bad.name" })
        {
            try { root.GetCompanionFilePath("main.log", name); throw new Exception("Invalid ACT companion name accepted"); }
            catch (ArgumentException) { }
        }
        // Bind actual clock properties used by the inspected companion gate, without executing its UI-dependent thread.
        var fields = BindingFlags.Instance | BindingFlags.NonPublic;
        typeof(FormActMain).GetField("estimatedTimer", fields)?.SetValue(root, new Stopwatch());
        typeof(FormActMain).GetField("logGate", fields)?.SetValue(root, new object());
        var epoch = DateTime.UnixEpoch;
        root.LastKnownTime = epoch;
        root.LastKnownTime = DateTime.MinValue;
        if (root.LastKnownTime != epoch || root.LastEstimatedTime < epoch) throw new Exception("ACT undated clock contract changed");
        root.LastKnownTime = epoch.AddSeconds(-1);
        if (root.LastKnownTime != epoch.AddSeconds(-1)) throw new Exception("ACT main clock assignment cannot move backward");
        return "Companion paths";
    }
}
