using System.Reflection;
using System.Runtime.Loader;

namespace Bxivact.Bridge;

internal static class Platform
{
    internal static void Prepare()
    {
        var bridge = Assembly.GetExecutingAssembly();
        var directory = Path.GetDirectoryName(bridge.Location)!;
        // Costura byte-loaded dependencies bind through Default, outside the plugin context.
        foreach (var name in new[] { "System.Windows.Extensions.dll", "System.Security.Permissions.dll" })
            AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.Combine(directory, name));
        if (OperatingSystem.IsLinux())
        {
            var context = AssemblyLoadContext.GetLoadContext(bridge)!;
            context.LoadFromAssemblyPath(Path.Combine(directory, "System.Windows.Forms.dll"));
            context.LoadFromAssemblyPath(Path.Combine(directory, "System.Windows.Forms.DataVisualization.dll"));
        }
    }
}
