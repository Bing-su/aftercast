using System.IO.Compression;
using System.Reflection;
using System.Runtime.Loader;
using System.Security.Cryptography;

namespace Bxivact.Bridge;

// Pin the inspected bytes; never guess offsets for an unknown plugin build.
internal static class LinuxPluginCompatibility
{
    internal static bool IsKnownPlugin(byte[] hash) => OperatingSystem.IsLinux()
        && Convert.ToHexStringLower(hash) == "af1031cdba8eeba8f53dd9d618e58d62d4919bfa77db9b31c568bd04cb359093";

    internal static byte[] RewriteMemory(byte[] original)
    {
        if (original.Length != 70144 || Convert.ToHexStringLower(SHA256.HashData(original)) != "0363753efe0d7ae52c0de73653a5436cf83cf655ca47515ce7aa33908f82d897")
            throw new InvalidOperationException("Unsupported FFXIV memory assembly for Linux compatibility");
        // REA method 0x06000096: tiny header at 9736, followed by 32 IL bytes.
        const int ilOffset = 9737;
        if (Convert.ToHexStringLower(SHA256.HashData(original.AsSpan(ilOffset, 32))) != "7879a598366bd64b02ddf60262f86130d8e4b73fd8990a8293417b9e467129e1")
            throw new InvalidOperationException("FFXIV administrator-check IL does not match the inspected build");
        var rewritten = (byte[])original.Clone();
        // No Windows administrator token exists on Linux; retain the original getter/setter and report false.
        Array.Fill(rewritten, (byte)0, ilOffset + 6, 20);
        rewritten[ilOffset + 6] = 0x16; // ldc.i4.0, then nineteen nops before set_IsAdministrator.
        return rewritten;
    }

    internal static Assembly LoadMemory(AssemblyLoadContext context, Assembly plugin)
    {
        using var resource = plugin.GetManifestResourceStream("costura.ffxiv_act_plugin.memory.dll.compressed")
            ?? throw new InvalidOperationException("Pinned FFXIV plugin has no embedded memory assembly");
        using var compressed = new DeflateStream(resource, CompressionMode.Decompress);
        // Bound decompression before checking hashes, even for an inspected resource.
        var bytes = new byte[70144];
        compressed.ReadExactly(bytes);
        if (compressed.ReadByte() != -1)
            throw new InvalidOperationException("Embedded FFXIV memory assembly exceeds its pinned size");
        using var image = new MemoryStream(RewriteMemory(bytes), false);
        return context.LoadFromStream(image);
    }
}
