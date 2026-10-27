using System.Reflection;
using System.Runtime.ExceptionServices;
using Advanced_Combat_Tracker;

internal static class CompanionCatalogChecks
{
    private static readonly Type Catalog = typeof(ActGlobals).Assembly.GetType("Advanced_Combat_Tracker.CompanionLogCatalog", true)!;
    private static (string Name, string Path)[] Discover(string path)
    {
        try
        {
            var values = (System.Collections.IEnumerable)Catalog.GetMethod("Discover", BindingFlags.Static | BindingFlags.NonPublic)!.Invoke(null, [path])!;
            return values.Cast<object>().Select(value => ((string)value.GetType().GetProperty("Name")!.GetValue(value)!,
                (string)value.GetType().GetProperty("Path")!.GetValue(value)!)).ToArray();
        }
        catch (TargetInvocationException error) when (error.InnerException is not null)
        {
            ExceptionDispatchInfo.Capture(error.InnerException).Throw();
            throw;
        }
    }
    private static void Check(bool value, string message)
    {
        if (!value) throw new Exception("Companion catalog: " + message);
    }
    private static void Reject<T>(Action action) where T : Exception
    {
        try { action(); throw new Exception("Expected " + typeof(T).Name); }
        catch (T) { }
    }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-companion-catalog-");
        try
        {
            var main = Path.Combine(directory.FullName, "network[1].log");
            var root = (FormActMain)System.Runtime.CompilerServices.RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
            GC.SuppressFinalize(root);
            Check(Discover(main).Length == 0, "missing companion directory");
            foreach (var name in new[] { "../escape", "a/b", "a\\b", "a:stream", "a*", "a?", "a\0", "a\n" })
                Reject<ArgumentException>(() => root.GetCompanionFilePath(main, name));
            var companions = Directory.CreateDirectory(Path.Combine(directory.FullName, "CompanionLogs"));
            foreach (var filename in new[] { "network[1].Zulu.log", "NETWORK[1].alpha.LOG", "network1.other.log", "network[1].wrong.txt", "network[1].bad.extra.log", "network[1]..log" })
                File.WriteAllText(Path.Combine(companions.FullName, filename), "synthetic");
            Directory.CreateDirectory(Path.Combine(companions.FullName, "network[1].directory.log"));
            var nested = Directory.CreateDirectory(Path.Combine(companions.FullName, "nested"));
            File.WriteAllText(Path.Combine(nested.FullName, "network[1].nested.log"), "ignored");
            var found = Discover(main);
            Check(found.Select(value => value.Name).SequenceEqual(["alpha", "Zulu"]), "literal/case/name matching or order");
            Check(found.All(value => Path.GetDirectoryName(value.Path) == companions.FullName), "search escaped the sibling directory");
            if (OperatingSystem.IsLinux())
            {
                // Only synthetic in-root targets are used; the resolver must reject even harmless links.
                var link = Path.Combine(companions.FullName, "network[1].link.log");
                File.CreateSymbolicLink(link, found[0].Path);
                Reject<IOException>(() => Discover(main));
                File.Delete(link);
                File.WriteAllText(Path.Combine(companions.FullName, "network[1].ALPHA.log"), "ambiguous");
                Reject<InvalidDataException>(() => Discover(main));
                File.Delete(Path.Combine(companions.FullName, "network[1].ALPHA.log"));
                var alias = Path.Combine(directory.FullName, "alias");
                Directory.CreateDirectory(alias);
                Directory.CreateSymbolicLink(Path.Combine(alias, "CompanionLogs"), companions.FullName);
                Reject<IOException>(() => Discover(Path.Combine(alias, "network[1].log")));
                var broken = Directory.CreateDirectory(Path.Combine(directory.FullName, "broken"));
                Directory.CreateSymbolicLink(Path.Combine(broken.FullName, "CompanionLogs"), Path.Combine(directory.FullName, "absent-target"));
                Reject<IOException>(() => Discover(Path.Combine(broken.FullName, "network[1].log")));
            }
            var blocked = Directory.CreateDirectory(Path.Combine(directory.FullName, "blocked"));
            File.WriteAllText(Path.Combine(blocked.FullName, "CompanionLogs"), "not a directory");
            Reject<IOException>(() => Discover(Path.Combine(blocked.FullName, "main.log")));
            // Fail instead of silently truncating a matching set that cannot be admitted as a bounded operation.
            for (var index = 0; index < 65; index++) File.WriteAllText(Path.Combine(companions.FullName, $"other.c{index}.log"), "");
            Reject<InvalidDataException>(() => Discover(Path.Combine(directory.FullName, "other.log")));
            for (var index = 0; index < 4097; index++) File.WriteAllText(Path.Combine(companions.FullName, $"unrelated-{index}"), "");
            Reject<InvalidDataException>(() => Discover(Path.Combine(directory.FullName, "missing.log")));
        }
        finally { directory.Delete(true); }
        Console.WriteLine("Companion naming, literal bounded sibling discovery, ambiguous names and symlink rejection passed.");
    }
}
