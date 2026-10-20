using System.Reflection;
using System.Runtime.Loader;
using System.Security.Cryptography;
using Advanced_Combat_Tracker;

namespace Bxivact.Bridge;

// Share contracts from the actual bridge context, which netcorehost may isolate from Default.
internal sealed class PluginContext(string path, byte[] hash) : AssemblyLoadContext("bxivact:" + path)
{
    private readonly AssemblyDependencyResolver resolver = new(path);
    protected override Assembly? Load(AssemblyName name)
    {
        var shared = new[] { typeof(IActPluginV1).Assembly, typeof(System.Windows.Forms.TabPage).Assembly }
            .FirstOrDefault(assembly => assembly.GetName().Name == name.Name);
        if (shared is not null)
        {
            if (name.Name == "Advanced Combat Tracker" && name.FullName != shared.GetName().FullName)
                throw new InvalidOperationException("Unsupported ACT reference: " + name.FullName);
            return shared;
        }
        // The chart contract must use the same Control identity as the shared WinForms adapter.
        if (OperatingSystem.IsLinux() && name.Name == "System.Windows.Forms.DataVisualization")
            return GetLoadContext(typeof(PluginContext).Assembly)!.LoadFromAssemblyName(name);
        if (name.Name == "FFXIV_ACT_Plugin.Memory" && LinuxPluginCompatibility.IsKnownPlugin(hash))
            return LinuxPluginCompatibility.LoadMemory(this, Assemblies.First(assembly => assembly.Location == path));
        // Share these with both file-loaded and Costura byte-loaded dependencies.
        if (name.Name is "System.Security.Permissions" or "System.Windows.Extensions")
            return Default.LoadFromAssemblyName(name);
        var resolved = resolver.ResolveAssemblyToPath(name);
        var sibling = Path.Combine(Path.GetDirectoryName(path)!, name.Name + ".dll");
        if (resolved is not null) return LoadFromAssemblyPath(resolved);
        if (File.Exists(sibling)) return LoadFromAssemblyPath(sibling);
        return null; // Let framework and Costura resolution run when no file exists.
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
        var hash = SHA256.HashData(File.ReadAllBytes(path));
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
            static bool Eligible(Type candidate) => candidate.IsVisible && candidate.IsClass && !candidate.IsAbstract
                && !candidate.ContainsGenericParameters && typeof(IActPluginV1).IsAssignableFrom(candidate)
                && candidate.GetConstructor(Type.EmptyTypes) is not null;
            Type candidate;
            if (typeName is not null)
            {
                candidate = assembly.GetType(typeName, true, false)!;
                if (!Eligible(candidate))
                    throw new InvalidOperationException("Type must be a public concrete IActPluginV1 with a public parameterless constructor");
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
