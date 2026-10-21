using Advanced_Combat_Tracker;

namespace Aftercast.Bridge;

internal static class OverlayPluginLifetime
{
    internal static void StopEventSources(IActPluginV1 plugin)
    {
        if (plugin.GetType().FullName != "RainbowMage.OverlayPlugin.PluginLoader") return;
        var container = plugin.GetType().GetProperty("Container")!.GetValue(plugin);
        if (container is null) return;
        var registryType = container.GetType().Assembly.GetType("RainbowMage.OverlayPlugin.Registry", true)!;
        var registry = container.GetType().GetMethod("Resolve", new[] { typeof(Type) })!.Invoke(container, new object[] { registryType })!;
        var sources = ((System.Collections.IEnumerable)registryType.GetProperty("EventSources")!.GetValue(registry)!).Cast<object>().ToArray();
        var failures = new List<Exception>();
        // ACT normally exits its process; an embedded host must stop addon-specific timers/hooks before disposing their container.
        foreach (var source in sources)
            try { source.GetType().GetMethod("Stop")!.Invoke(source, null); }
            catch (Exception error) { failures.Add(error); }
        if (failures.Count != 0) throw new AggregateException("OverlayPlugin event source shutdown failed", failures);
    }
}
