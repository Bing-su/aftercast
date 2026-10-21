using System.Runtime.CompilerServices;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class PluginMetadataProbe
{
    public static string Run()
    {
        // Read data contracts without starting ACT's main application or plugin manager.
        var root = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        GC.SuppressFinalize(root);
        var plugins = new List<ActPluginData>();
        root.ActPlugins = plugins;
        if (!ReferenceEquals(root.ActPlugins, plugins) || root.IsActClosing || root.GetVersion() != new Version(3, 8, 5, 288))
            throw new Exception("Plugin metadata reference mismatch");
        if (typeof(FormActMain).GetMethod("NotificationAdd", new[] { typeof(string), typeof(string), typeof(EventHandler), typeof(object) }) is null)
            throw new Exception("Notification method signature mismatch");
        return "Plugin metadata";
    }
}
