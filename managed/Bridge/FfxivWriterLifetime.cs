using Advanced_Combat_Tracker;

namespace Aftercast.Bridge;

internal static class FfxivWriterLifetime
{
    internal static VendorWriterObserver? Capture(IActPluginV1 plugin, Action<string> report)
    {
        if (!PluginLoader.IsPinnedFfxiv(plugin.GetType().Assembly)) return null;
        // This field is null before data collection has been constructed, including early Init failure.
        var collection = plugin.GetType().GetField("_dataCollection",
            System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance)?.GetValue(plugin);
        if (collection is null) return null;
        return new(VendorWriterObserver.Field(collection, "_logOutput"),
            VendorWriterObserver.Field(collection, "_settingsMediator"),
            new[] { VendorWriterObserver.ThreadOf(VendorWriterObserver.Field(collection, "_scanMemory")),
                    VendorWriterObserver.ThreadOf(VendorWriterObserver.Field(collection, "_monitor")) }, report);
    }
}
