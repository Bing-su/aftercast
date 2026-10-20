using System.Collections;
using System.Reflection;
using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;
using System.Runtime.Loader;
using System.Security.Cryptography;
using System.Text;
using Bxivact.Bridge;

internal static class ResourceCompatibilityChecks
{
    private const string Prefix = "FFXIV_ACT_Plugin.Resource.";
    private const string OverridePrefix = "ffxiv_act_plugin.resource.generated.override.global_";

    internal static void Run(string artifactPath)
    {
        artifactPath = Path.GetFullPath(artifactPath);
        var original = File.ReadAllBytes(artifactPath);
        var before = SHA256.HashData(original);
        var rewritten = LinuxPluginCompatibility.RewriteResource(original);
        Check(!ReferenceEquals(original, rewritten) && rewritten.Length == original.Length, "same-size owned image");
        Check(SHA256.HashData(original).AsSpan().SequenceEqual(before), "original array unchanged");
        Check(Convert.ToHexStringLower(SHA256.HashData(rewritten)) == "3767471753a70412b92af4e0978fec74f92eb04e9c1aad4f97207055f9a9b780", "rewritten image hash");
        CheckLayout(original, rewritten);
        Rejected([]);
        Rejected(rewritten);
        var unknown = (byte[])original.Clone();
        unknown[100] ^= 1;
        Rejected(unknown);
        var context = new AssemblyLoadContext("bxivact-resource-compatibility-check", true);
        context.Resolving += (_, name) =>
        {
            var expected = name.Name switch
            {
                "FFXIV_ACT_Plugin.Common" => "9c3ce07598b6e77a45a6e346bf6c4efbbb09b9cc61a32534f23bc5adcdb03cb1",
                "FFXIV_ACT_Plugin.Config" => "f965992b4f520ae9aabc2158f925ae4044be647113bc1e7770abb9a6e3b20b2e",
                _ => null
            };
            if (expected is null) return null;
            // The supplied SDK ZIP can be extracted into ordinary siblings or literal SDK\-prefixed filenames on Linux.
            var directory = Path.GetDirectoryName(artifactPath)!;
            var sibling = new[] { Path.Combine(directory, name.Name + ".dll"), Path.Combine(directory, "SDK\\" + name.Name + ".dll") }
                .FirstOrDefault(File.Exists) ?? throw new FileNotFoundException("Pinned resource companion is missing", name.Name);
            Check(Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(sibling))) == expected, "pinned resource companion " + name.Name);
            return context.LoadFromAssemblyPath(sibling);
        };
        try
        {
            using var image = new MemoryStream(rewritten, false);
            var assembly = context.LoadFromStream(image);
            CheckResources(assembly);
        }
        finally { context.Unload(); }
        Check(SHA256.HashData(File.ReadAllBytes(artifactPath)).AsSpan().SequenceEqual(before), "source artifact unchanged");
    }

    private static void CheckLayout(byte[] original, byte[] rewritten)
    {
        using var first = new PEReader(new MemoryStream(original, false));
        using var second = new PEReader(new MemoryStream(rewritten, false));
        var md = first.GetMetadataReader();
        var method = md.GetMethodDefinition(MetadataTokens.MethodDefinitionHandle(0x68));
        var oldBody = first.GetMethodBody(method.RelativeVirtualAddress);
        var body = second.GetMethodBody(method.RelativeVirtualAddress);
        Check(Convert.ToHexStringLower(SHA256.HashData(body.GetILBytes()!)) ==
            "f4f48266e0b23c7d61c12cd3cf1b10c02d2dc904f33b4e1230b54cfe8bf2d2ba", "patched 943-byte IL hash");
        Check(body.GetILBytes()!.Length == 943 && body.MaxStack == 6 && body.LocalSignature == oldBody.LocalSignature &&
            body.LocalVariablesInitialized == oldBody.LocalVariablesInitialized && body.ExceptionRegions.SequenceEqual(oldBody.ExceptionRegions), "method layout and EH");
        Check(body.GetILBytes()!.AsSpan(86).SequenceEqual(oldBody.GetILBytes()!.AsSpan(86)), "all IL86+ unchanged");
        // Bind the replacement's literal/member operands to actual metadata instead of trusting only its byte window.
        Check(md.GetUserString(MetadataTokens.UserStringHandle(0x445)) == "\r" &&
            md.GetUserString(MetadataTokens.UserStringHandle(0x449)) == "\n", "existing CR/LF literal source");
        var concat = md.GetMemberReference(MetadataTokens.MemberReferenceHandle(0x21));
        var declaring = md.GetTypeReference((TypeReferenceHandle)concat.Parent);
        Check(md.GetString(declaring.Namespace) == "System" && md.GetString(declaring.Name) == "String" &&
            md.GetString(concat.Name) == "Concat" && md.GetBlobBytes(concat.Signature).AsSpan()
                .SequenceEqual(new byte[] { 0, 2, 0x0e, 0x0e, 0x0e }), "existing String.Concat(string,string) member source");
        for (var index = 0; index < original.Length; index++)
            if (index < 3948 + 43 || index >= 3948 + 86) Check(original[index] == rewritten[index], "only the 43-byte window changes");
    }

    private static void CheckResources(Assembly assembly)
    {
        var type = assembly.GetType(Prefix + "ResourceFile", true)!;
        var names = assembly.GetManifestResourceNames().Where(name => name.EndsWith(".txt", StringComparison.Ordinal)).ToArray();
        var texts = names.ToDictionary(name => name, name => ReadText(assembly, name));
        var overrides = names.Where(name => name.StartsWith(OverridePrefix, StringComparison.OrdinalIgnoreCase)).ToArray();
        var singleton = assembly.ManifestModule.ResolveField(0x0400007f)!;
        var selectorCache = assembly.ManifestModule.ResolveField(0x04000080)!;
        var target = singleton.GetValue(null);
        Check(target is not null && selectorCache.GetValue(null) is null, "original singleton and empty first cache");
        var dictionaries = 0;
        var failures = 0;
        var changes = 0;
        var elapsed = System.Diagnostics.Stopwatch.StartNew();
        foreach (var name in names)
        foreach (var hex in new[] { false, true })
        {
            var shortName = name[Prefix.Length..];
            SortedDictionary<uint, string>? expected = null;
            try { expected = WindowsReference(shortName, texts[name], hex, texts, overrides, ref changes); }
            catch (Exception error) when (error is FormatException or OverflowException or ArgumentException) { }
            var result = ReadDictionary(type, shortName, hex);
            if (expected is null)
            {
                // Wrong radix and nonnumeric override resources retain the genuine reader's caught-parse/null-source failure.
                Check(result.Error is ArgumentNullException argument && argument.ParamName == "source", shortName + " invalid numeric API path");
                failures++;
                continue;
            }
            Check(result.Error is null && result.Values is not null, shortName + " dictionary read");
            Check(result.Values.Count == expected.Count, shortName + " dictionary count");
            foreach (var pair in expected)
                Check(result.Values.Contains(pair.Key) && Equals(result.Values[pair.Key], pair.Value), shortName + " exact value " + pair.Key);
            dictionaries++;
        }
        var territory = ReadDictionary(type, "Generated.TerritoryList_English.txt", false);
        Check(territory.Error is null && (string)territory.Values![129u]! == "Limsa Lominsa Lower Decks", "exact territory delimiter");
        Check(ReferenceEquals(singleton.GetValue(null), target) && selectorCache.GetValue(null) is null, "singleton/cache state unchanged");
        Check(names.Length == 53 && dictionaries == 44 && failures == 62 && overrides.Length == 23 && changes == 645, "complete resource/radix/override coverage");
        Console.WriteLine($"Pinned resources: 53; radix cases: 106; exact dictionaries: {dictionaries}; existing failures: {failures}; " +
            $"embedded overrides: {overrides.Length}; replacements: {changes}; elapsedMs: {elapsed.ElapsedMilliseconds}");
    }

    private static (IDictionary? Values, Exception? Error) ReadDictionary(Type type, string name, bool hex)
    {
        try
        {
            var resource = Activator.CreateInstance(type, name, hex)!;
            return ((IDictionary)type.GetProperty("Dictionary")!.GetValue(resource)!, null);
        }
        catch (TargetInvocationException error) when (error.InnerException is not null) { return (null, error.InnerException); }
    }

    private static string ReadText(Assembly assembly, string name)
    {
        using var stream = assembly.GetManifestResourceStream(name)!;
        using var reader = new StreamReader(stream, new UTF8Encoding(false, true), true);
        return reader.ReadToEnd();
    }

    private static SortedDictionary<uint, string> WindowsReference(string file, string text, bool hex,
        Dictionary<string, string> texts, string[] overrides, ref int changes)
    {
        // Reproduce retained selectors/Join independently, using exact Windows separators and preserving embedded lone LF.
        var result = new SortedDictionary<uint, string>(text.Split(["\r\n"], StringSplitOptions.RemoveEmptyEntries)
            .Select(line => line.Split(["|"], StringSplitOptions.RemoveEmptyEntries)).Where(parts => parts.Length == 2)
            .ToDictionary(parts => Convert.ToUInt32(parts[0], hex ? 16 : 10), parts => parts[1]));
        if (!file.StartsWith("generated.statuslist_", StringComparison.OrdinalIgnoreCase) &&
            !file.StartsWith("generated.actionlist_", StringComparison.OrdinalIgnoreCase)) return result;
        foreach (var name in overrides)
        {
            var replacements = texts[name].Split(["\r\n", "\r", "\n"], StringSplitOptions.RemoveEmptyEntries)
                .Select(line => line.Split(["|"], StringSplitOptions.RemoveEmptyEntries)).Where(parts => parts.Length == 2)
                .ToDictionary(parts => parts[0], parts => parts[1]);
            var updates = replacements.Join(result, replacement => replacement.Key.ToLowerInvariant(), value => value.Value.ToLowerInvariant(),
                (replacement, value) => new KeyValuePair<uint, string>(value.Key, replacement.Value)).ToList();
            foreach (var update in updates)
            {
                if (result[update.Key] != update.Value) changes++;
                result[update.Key] = update.Value;
            }
        }
        return result;
    }

    private static void Rejected(byte[] image)
    {
        try { LinuxPluginCompatibility.RewriteResource(image); }
        catch (InvalidOperationException error) when (error.Message.Contains("Unsupported FFXIV resource assembly", StringComparison.Ordinal)) { return; }
        throw new InvalidOperationException("Unknown or already-rewritten resource image was accepted");
    }
    private static void Check([System.Diagnostics.CodeAnalysis.DoesNotReturnIf(false)] bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException("Resource compatibility check failed: " + message);
    }
}
