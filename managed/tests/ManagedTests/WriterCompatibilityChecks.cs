using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;
using System.Security.Cryptography;
using Bxivact.Bridge;

internal static class WriterCompatibilityChecks
{
    internal static void Run()
    {
        const string path = "binaries/FFXIV_ACT_Plugin_SDK_3.0.3.3/SDK\\FFXIV_ACT_Plugin.Logfile.dll";
        var original = File.ReadAllBytes(path);
        var before = SHA256.HashData(original);
        var rewritten = LinuxPluginCompatibility.RewriteLogfile(original);
        Check(SHA256.HashData(original).AsSpan().SequenceEqual(before) && SHA256.HashData(File.ReadAllBytes(path)).AsSpan().SequenceEqual(before), "Original writer changed.");
        Check(!ReferenceEquals(original, rewritten) && original.Length == rewritten.Length, "Rewrite must own a same-size image.");
        Check(Convert.ToHexStringLower(SHA256.HashData(rewritten)) == "2530772a8582b7fe5d180964f42eebd49440fbc8dbc9d4f10cd9d859dc9364fa", "Unexpected rewritten writer image.");
        using var originalPe = new PEReader(new MemoryStream(original, false));
        using var rewrittenPe = new PEReader(new MemoryStream(rewritten, false));
        var metadata = rewrittenPe.GetMetadataReader();
        Check(metadata.GetGuid(metadata.GetModuleDefinition().Mvid) == new Guid("ff6af951-0fd5-413c-b114-d048ca6b23b4"), "Writer MVID changed.");
        var method = metadata.GetMethodDefinition(MetadataTokens.MethodDefinitionHandle(0x48));
        Check(metadata.GetString(method.Name) == "Run" && metadata.GetBlobBytes(method.Signature).AsSpan().SequenceEqual(new byte[] { 0x20, 1, 1, 0x1c }), "Writer method signature changed.");
        var oldBody = originalPe.GetMethodBody(method.RelativeVirtualAddress);
        var body = rewrittenPe.GetMethodBody(method.RelativeVirtualAddress);
        Check(body.GetILBytes()!.Length == 485 && body.MaxStack == oldBody.MaxStack && body.LocalSignature == oldBody.LocalSignature &&
            body.LocalVariablesInitialized == oldBody.LocalVariablesInitialized && body.ExceptionRegions.SequenceEqual(oldBody.ExceptionRegions), "Writer header/EH layout changed.");
        var il = body.GetILBytes()!;
        Check(il.AsSpan(299, 19).SequenceEqual(new byte[] { 0x2c, 2, 0xde, 63, 0x12, 0, 0x28, 0x58, 0, 0, 0x0a, 0x39, 0x42, 0xff, 0xff, 0xff, 0xde, 49, 0 }) &&
            il[284] == 18 && il[341] == 217, "Unexpected cancellation CFG.");
        // The sole mutation windows exclude the method header, folder block and every exception-region table.
        for (var index = 0; index < original.Length; index++)
            if (index != 5204 + 284 && index != 5204 + 341 && (index < 5204 + 299 || index > 5204 + 317))
                Check(original[index] == rewritten[index], "Rewrite changed an unrelated byte.");
        Rejected([]);
        Rejected(rewritten);
        var changed = (byte[])original.Clone();
        changed[100] ^= 1;
        Rejected(changed);
        Console.WriteLine("Pinned original writer cancellation image and rejection guards passed.");
    }

    private static void Rejected(byte[] image)
    {
        try { LinuxPluginCompatibility.RewriteLogfile(image); }
        catch (InvalidOperationException error) when (error.Message.Contains("Unsupported FFXIV logfile assembly", StringComparison.Ordinal)) { return; }
        throw new InvalidOperationException("Unknown/already-rewritten writer image was accepted.");
    }
    private static void Check(bool value, string message)
    {
        if (!value) throw new InvalidOperationException(message);
    }
}
