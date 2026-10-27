using System.Runtime.CompilerServices;
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
        return "Companion paths";
    }
}
