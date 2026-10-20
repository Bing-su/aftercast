using System.IO.Compression;
using System.Reflection;
using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;
using System.Runtime.Loader;
using System.Security.Cryptography;

namespace Bxivact.Bridge;

// Pin the inspected bytes; never guess offsets for an unknown plugin build.
internal static partial class LinuxPluginCompatibility
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
        => LoadEmbedded(context, plugin, "memory", 70144, RewriteMemory);

    internal static Assembly LoadLogfile(AssemblyLoadContext context, Assembly plugin)
        => LoadEmbedded(context, plugin, "logfile", 26112, RewriteLogfile);

    internal static Assembly LoadResource(AssemblyLoadContext context, Assembly plugin)
        => LoadEmbedded(context, plugin, "resource", 8671232, RewriteResource);

    internal static byte[] RewriteLogfile(byte[] original)
    {
        if (original.Length != 26112 || Convert.ToHexStringLower(SHA256.HashData(original)) != "2f4faa819df144ca70a17c17e79e45c9cdecf413c98c12fc42b51d73af10b5e7")
            throw new InvalidOperationException("Unsupported FFXIV logfile assembly for Linux compatibility");
        using var pe = new PEReader(new MemoryStream(original, false));
        var metadata = pe.GetMetadataReader();
        var method = metadata.GetMethodDefinition(MetadataTokens.MethodDefinitionHandle(0x48));
        var declaring = metadata.GetTypeDefinition(method.GetDeclaringType());
        var body = pe.GetMethodBody(method.RelativeVirtualAddress);
        var il = body.GetILBytes()!;
        if (metadata.GetGuid(metadata.GetModuleDefinition().Mvid) != new Guid("ff6af951-0fd5-413c-b114-d048ca6b23b4") ||
            metadata.GetString(declaring.Namespace) != "FFXIV_ACT_Plugin.Logfile" || metadata.GetString(declaring.Name) != "LogOutput" ||
            metadata.GetString(method.Name) != "Run" || !metadata.GetBlobBytes(method.Signature).AsSpan().SequenceEqual(new byte[] { 0x20, 1, 1, 0x1c }) ||
            method.RelativeVirtualAddress != 12872 || il.Length != 485 || body.MaxStack != 5 || !body.LocalVariablesInitialized ||
            MetadataTokens.GetToken(body.LocalSignature) != 0x1100000a ||
            Convert.ToHexStringLower(SHA256.HashData(il)) != "253503df897ee91a7aaebaee04e3009cf55691a703c6aacb946e3c1125440972")
            throw new InvalidOperationException("FFXIV logfile Run IL does not match the inspected build");
        var expectedRegions = new[]
        {
            (ExceptionRegionKind.Finally, 120, 224, 344, 22, 0),
            (ExceptionRegionKind.Finally, 92, 276, 368, 10, 0),
            (ExceptionRegionKind.Catch, 77, 303, 380, 3, 0x01000028),
            (ExceptionRegionKind.Catch, 77, 303, 383, 37, 0x01000027),
            (ExceptionRegionKind.Catch, 77, 303, 420, 45, 0x01000023)
        };
        if (!body.ExceptionRegions.Select(region => (region.Kind, region.TryOffset, region.TryLength,
                region.HandlerOffset, region.HandlerLength, region.CatchType.IsNil ? 0 : MetadataTokens.GetToken(region.CatchType)))
            .SequenceEqual(expectedRegions))
            throw new InvalidOperationException("FFXIV logfile Run exception regions do not match the inspected build");
        // Pinned fat header at 5192 has twelve bytes; no header, metadata or exception table changes are needed.
        const int ilOffset = 5204;
        if (!original.AsSpan(ilOffset, il.Length).SequenceEqual(il))
            throw new InvalidOperationException("FFXIV logfile Run file offset does not match the inspected build");
        var rewritten = (byte[])original.Clone();
        // Reclaim three bytes with leave.s301→366, move the cancellation check to303 and explicitly leave315→366.
        // Both leaves use the existing writer/stream disposal chain; false still dequeues125 and folder checks stay intact.
        byte[] replacement = [0x2c, 2, 0xde, 63, 0x12, 0, 0x28, 0x58, 0, 0, 0x0a, 0x39, 0x42, 0xff, 0xff, 0xff, 0xde, 49, 0];
        replacement.CopyTo(rewritten, ilOffset + 299);
        rewritten[ilOffset + 284] = 18; // br.s283→303 after a successful write/flush.
        rewritten[ilOffset + 341] = 217; // brtrue.s340→303 after the initial folder comparison.
        return rewritten;
    }

    private static Assembly LoadEmbedded(AssemblyLoadContext context, Assembly plugin, string name, int size, Func<byte[], byte[]> rewrite)
    {
        using var resource = plugin.GetManifestResourceStream($"costura.ffxiv_act_plugin.{name}.dll.compressed")
            ?? throw new InvalidOperationException($"Pinned FFXIV plugin has no embedded {name} assembly");
        using var compressed = new DeflateStream(resource, CompressionMode.Decompress);
        // Bound decompression before checking hashes, even for an inspected resource.
        var bytes = new byte[size];
        compressed.ReadExactly(bytes);
        if (compressed.ReadByte() != -1)
            throw new InvalidOperationException($"Embedded FFXIV {name} assembly exceeds its pinned size");
        using var image = new MemoryStream(rewrite(bytes), false);
        return context.LoadFromStream(image);
    }
}
