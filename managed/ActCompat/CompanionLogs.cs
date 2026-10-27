namespace Aftercast.ActCompat
{
    public sealed record CompanionCandidate(string Name, string Path);
}

namespace Advanced_Combat_Tracker
{

public partial class FormActMain
{
    // Preserve ACT's sibling naming contract without allowing a plugin name to escape that directory.
    public string GetCompanionFilePath(string MainLogPath, string CompanionLogName)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(MainLogPath);
        CompanionLogCatalog.ValidateName(CompanionLogName);
        var stem = Path.GetFileNameWithoutExtension(MainLogPath);
        if (string.IsNullOrWhiteSpace(stem)) throw new ArgumentException("Main log must name a file", nameof(MainLogPath));
        return Path.Combine(Path.GetDirectoryName(MainLogPath)!, "CompanionLogs", stem + "." + CompanionLogName + Path.GetExtension(MainLogPath));
    }
}

internal sealed record CompanionLogSource(string Name, string Path);

// Discovery is a bounded proposal, not permission to follow paths later without owned admission handles.
internal static class CompanionLogCatalog
{
    private const int MaximumDirectoryEntries = 4096;
    private const int MaximumCompanions = 64;
    internal static void ValidateName(string name)
    {
        if (string.IsNullOrWhiteSpace(name)) throw new ArgumentNullException(nameof(name), "Companion name cannot be blank");
        if (name.Length > 256 || name.Any(character => char.IsControl(character) || ".\\/:*?\"<>|".Contains(character)))
            throw new ArgumentException("Companion name must be a portable single filename component without periods", nameof(name));
    }

    internal static IReadOnlyList<CompanionLogSource> Discover(string mainPath)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(mainPath);
        var fullPath = System.IO.Path.GetFullPath(mainPath);
        var stem = System.IO.Path.GetFileNameWithoutExtension(fullPath);
        if (string.IsNullOrWhiteSpace(stem)) throw new ArgumentException("Main log must name a file", nameof(mainPath));
        var extension = System.IO.Path.GetExtension(fullPath);
        var directory = System.IO.Path.Combine(System.IO.Path.GetDirectoryName(fullPath)!, "CompanionLogs");
        FileAttributes directoryAttributes;
        try { directoryAttributes = File.GetAttributes(directory); }
        catch (DirectoryNotFoundException) { return Array.Empty<CompanionLogSource>(); }
        catch (FileNotFoundException) { return Array.Empty<CompanionLogSource>(); }
        if ((directoryAttributes & FileAttributes.ReparsePoint) != 0)
            throw new IOException("Companion directory cannot be a symbolic link or reparse point");
        if ((directoryAttributes & FileAttributes.Directory) == 0) throw new IOException("CompanionLogs must be a directory");
        var prefix = stem + ".";
        var found = new SortedDictionary<string, CompanionLogSource>(StringComparer.OrdinalIgnoreCase);
        var entries = 0;
        // Enumerate names literally: a main filename containing glob/regex metacharacters cannot widen the search.
        foreach (var path in Directory.EnumerateFileSystemEntries(directory))
        {
            if (++entries > MaximumDirectoryEntries) throw new InvalidDataException("Companion directory exceeds 4096 entries");
            var filename = System.IO.Path.GetFileName(path);
            if (!filename.StartsWith(prefix, StringComparison.OrdinalIgnoreCase) || !filename.EndsWith(extension, StringComparison.OrdinalIgnoreCase)) continue;
            var nameLength = filename.Length - prefix.Length - extension.Length;
            if (nameLength <= 0) continue;
            var name = filename.Substring(prefix.Length, nameLength);
            // Extra-dot names belong to another parent or are not ACT companion names.
            if (name.Contains('.')) continue;
            ValidateName(name);
            var attributes = File.GetAttributes(path);
            if ((attributes & FileAttributes.ReparsePoint) != 0) throw new IOException("Companion source cannot be a symbolic link or reparse point");
            if ((attributes & FileAttributes.Directory) != 0) continue;
            if (!found.TryAdd(name, new(name, path))) throw new InvalidDataException("Ambiguous companion name: " + name);
            if (found.Count > MaximumCompanions) throw new InvalidDataException("More than 64 companion sources");
        }
        return found.Values.ToArray();
    }
}

}
