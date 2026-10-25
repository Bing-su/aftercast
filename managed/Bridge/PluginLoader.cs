using System.Reflection;
using System.Runtime.Loader;
using System.Security.Cryptography;
using Advanced_Combat_Tracker;

namespace Aftercast.Bridge;

// Share contracts from the actual bridge context, which netcorehost may isolate from Default.
internal sealed class PluginContext(string path, byte[] hash) : AssemblyLoadContext("aftercast:" + path)
{
    private readonly AssemblyDependencyResolver resolver = new(path);
    private static readonly object contractGate = new();
    private static readonly Dictionary<string, Assembly> overlayContracts = new();
    internal static bool MatchesSharedContract(AssemblyName reference, AssemblyName actual) =>
        reference.Name == actual.Name && (reference.Version is null || actual.Version >= reference.Version)
        && (reference.CultureName ?? "") == (actual.CultureName ?? "")
        && (reference.Version is null && reference.GetPublicKeyToken() is not { Length: > 0 } ||
            reference.GetPublicKeyToken().AsSpan().SequenceEqual(actual.GetPublicKeyToken()));
    internal static bool SupportsActReference(AssemblyName reference)
    {
        var actual = typeof(IActPluginV1).Assembly.GetName();
        // Match ACT's shipped 2.x/3.x binding redirect, e.g. Benchmark's 3.5.0.273 reference.
        return reference.Name == actual.Name && reference.Version is { } version
            && version >= new Version(2, 0, 0, 0) && version <= new Version(3, 65535, 65535, 65535)
            && (reference.CultureName ?? "") == (actual.CultureName ?? "")
            && reference.GetPublicKeyToken().AsSpan().SequenceEqual(actual.GetPublicKeyToken());
    }
    protected override Assembly? Load(AssemblyName name)
    {
        var shared = name.Name switch
        {
            "Advanced Combat Tracker" => typeof(IActPluginV1).Assembly,
            "System.Windows.Forms" => typeof(System.Windows.Forms.TabPage).Assembly,
            _ => null
        };
        if (shared is not null)
        {
            if (name.Name == "Advanced Combat Tracker" && !SupportsActReference(name))
                throw new InvalidOperationException("Unsupported ACT reference: " + name.FullName);
            return shared;
        }
        // The chart contract must use the same Control identity as the shared WinForms adapter.
        if (OperatingSystem.IsLinux() && name.Name == "System.Windows.Forms.DataVisualization")
            return GetLoadContext(typeof(PluginContext).Assembly)!.LoadFromAssemblyName(name);
        if (name.Name is "FFXIV_ACT_Plugin.Memory" or "FFXIV_ACT_Plugin.Logfile" or "FFXIV_ACT_Plugin.Resource" && LinuxPluginCompatibility.IsKnownPlugin(hash))
        {
            var plugin = Assemblies.First(assembly => assembly.Location == path);
            return name.Name switch
            {
                "FFXIV_ACT_Plugin.Memory" => LinuxPluginCompatibility.LoadMemory(this, plugin),
                "FFXIV_ACT_Plugin.Logfile" => LinuxPluginCompatibility.LoadLogfile(this, plugin),
                _ => LinuxPluginCompatibility.LoadResource(this, plugin)
            };
        }
        // Share these with both file-loaded and Costura byte-loaded dependencies.
        if (name.Name is "System.Security.Permissions" or "System.Windows.Extensions")
            return Default.LoadFromAssemblyName(name);
        var resolved = resolver.ResolveAssemblyToPath(name);
        if (resolved is null)
        {
            var directory = Path.GetDirectoryName(path)!;
            var sibling = Path.Combine(directory, name.Name + ".dll");
            var library = Path.Combine(directory, "libs", name.Name + ".dll");
            // OverlayPlugin keeps private dependencies under libs; all loaders use this same fallback order.
            resolved = File.Exists(sibling) ? sibling : File.Exists(library) ? library : null;
        }
        if (name.Name == "HtmlRenderer" && resolved is not null &&
            OverlayRendererCompatibility.LoadIfKnown(this, resolved) is { } renderer) return renderer;
        // Addon signatures contain both the OverlayPlugin interfaces and Newtonsoft tokens; share their identities.
        if (name.Name is "OverlayPlugin.Common" or "Newtonsoft.Json")
        {
            lock (contractGate)
            {
                if (overlayContracts.TryGetValue(name.Name, out var contract))
                {
                    if (!MatchesSharedContract(name, contract.GetName())) throw new FileLoadException("Incompatible overlay contract: " + name);
                    return contract;
                }
                if (resolved is not null)
                {
                    if (!MatchesSharedContract(name, AssemblyName.GetAssemblyName(resolved)))
                        throw new FileLoadException("Incompatible overlay contract: " + name);
                    contract = LoadFromAssemblyPath(resolved);
                    overlayContracts.Add(name.Name, contract);
                    return contract;
                }
            }
        }
        // Let framework and Costura resolution run when no file exists.
        return resolved is null ? null : LoadFromAssemblyPath(resolved);
    }
    protected override nint LoadUnmanagedDll(string name)
    {
        var resolved = resolver.ResolveUnmanagedDllToPath(name);
        return resolved is null ? 0 : LoadUnmanagedDllFromPath(resolved);
    }
}

internal static class PluginLoader
{
    // Constructors can register global handlers; keep assemblies process-owned across sessions.
    private static readonly Dictionary<string, (byte[] Hash, Assembly Assembly)> assemblies = new(
        OperatingSystem.IsWindows() ? StringComparer.OrdinalIgnoreCase : StringComparer.Ordinal);
    private static Assembly Load(string path)
    {
        byte[] hash;
        using (var file = File.OpenRead(path)) hash = SHA256.HashData(file);
        if (assemblies.TryGetValue(path, out var previous))
        {
            if (!previous.Hash.AsSpan().SequenceEqual(hash))
                throw new InvalidOperationException("Plugin file changed after loading; restart the process before replacing plugins");
            return previous.Assembly;
        }
        var context = new PluginContext(path, hash);
        var assembly = context.LoadFromAssemblyPath(path);
        assemblies.Add(path, (hash, assembly));
        return assembly;
    }
    internal static object Create(string path, string? typeName)
    {
        var stage = "load";
        try
        {
            path = Path.GetFullPath(path);
            var assembly = Load(path);
            stage = "select type";
            static bool Eligible(Type candidate) => candidate.IsClass && !candidate.IsAbstract
                && !candidate.ContainsGenericParameters && typeof(IActPluginV1).IsAssignableFrom(candidate)
                && candidate.GetConstructor(Type.EmptyTypes) is not null;
            Type candidate;
            if (typeName is not null)
            {
                candidate = assembly.GetType(typeName, true, false)!;
                if (!Eligible(candidate))
                    throw new InvalidOperationException("Type must be a concrete IActPluginV1 with a public parameterless constructor");
            }
            else
            {
                var candidates = assembly.GetTypes().Where(Eligible).ToArray();
                if (candidates.Length != 1)
                    throw new InvalidOperationException($"Expected one constructible IActPluginV1, found {candidates.Length}; specify type_name");
                candidate = candidates[0];
            }
            stage = "construct " + candidate.FullName;
            return Activator.CreateInstance(candidate)!;
        }
        catch (Exception error)
        {
            var details = error is ReflectionTypeLoadException types
                ? string.Join("; ", types.LoaderExceptions.Select(failure => failure?.Message))
                : error.GetBaseException().Message;
            throw new InvalidOperationException($"Plugin {stage} failed for {path}: {details}", error);
        }
    }
}
