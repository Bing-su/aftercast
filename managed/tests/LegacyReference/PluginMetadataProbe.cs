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
        if (!ReferenceEquals(root.ActPlugins, plugins) || root.IsActClosing || root.InitActDone || root.GetVersion() != new Version(3, 8, 5, 288))
            throw new Exception("Plugin metadata reference mismatch");
        if (typeof(FormActMain).GetMethod("NotificationAdd", new[] { typeof(string), typeof(string), typeof(EventHandler), typeof(object) }) is null)
            throw new Exception("Notification method signature mismatch");
        // Match the exact fields consumed by OverlayPlugin; equal names with different UI identities still fail binding.
        if (typeof(ActPluginData).GetField("cbEnabled")?.FieldType != typeof(System.Windows.Forms.CheckBox) ||
            typeof(ActPluginData).GetField("lblPluginTitle")?.FieldType != typeof(System.Windows.Forms.Label) ||
            typeof(ActGlobals).GetField("oFormImportProgress")?.FieldType != typeof(FormImportProgress))
            throw new Exception("Overlay plugin metadata field signature mismatch");
        return "Plugin metadata";
    }
}
