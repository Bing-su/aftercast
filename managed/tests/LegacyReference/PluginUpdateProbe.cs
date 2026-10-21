using System.Runtime.CompilerServices;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class PluginUpdateProbe
{
    public static string Run()
    {
        var root = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        GC.SuppressFinalize(root);
        var text = root.PluginGetRemoteVersion(86);
        if (!Version.TryParse(text.Trim().TrimStart('v'), out var version) || version.Major == 0 && version.Minor == 0)
            throw new Exception("Official plugin version endpoint did not return a version");
        Console.WriteLine("Official ACT plugin 86 version: " + text);
        return "Plugin update query";
    }
}
