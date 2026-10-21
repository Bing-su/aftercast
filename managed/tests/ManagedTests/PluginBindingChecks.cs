using System.Reflection;
using Aftercast.Bridge;

internal static class PluginBindingChecks
{
    internal static void Run()
    {
        const string identity = "Advanced Combat Tracker, Culture=neutral, PublicKeyToken=a946b61e93d97868";
        foreach (var version in new[] { "2.0.0.0", "3.5.0.273", "3.8.5.288", "3.65535.65535.65535" })
            if (!PluginContext.SupportsActReference(new AssemblyName(identity) { Version = Version.Parse(version) }))
                throw new Exception("ACT binding redirect rejected " + version);
        // A redirect changes the version only; unrelated identities must never receive the shared ACT contract.
        foreach (var name in new[]
        {
            identity + ", Version=1.9.9.9",
            identity + ", Version=4.0.0.0",
            identity,
            "Advanced Combat Tracker, Version=3.5.0.273, Culture=neutral, PublicKeyToken=null",
            "Advanced Combat Tracker, Version=3.5.0.273, Culture=ko-KR, PublicKeyToken=a946b61e93d97868",
            "Other, Version=3.5.0.273, Culture=neutral, PublicKeyToken=a946b61e93d97868"
        })
            if (PluginContext.SupportsActReference(new AssemblyName(name)))
                throw new Exception("ACT binding redirect accepted " + name);
        Console.WriteLine("ACT binding redirect bounds, culture and signing identity checks passed.");
    }
}
