using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;
using System.Security.Cryptography;

namespace Bxivact.Bridge;

// Restore the original Windows delimiter without changing resource bytes, metadata tokens or exception boundaries.
internal static partial class LinuxPluginCompatibility
{
    internal static byte[] RewriteResource(byte[] original)
    {
        if (original.Length != 8671232 || Convert.ToHexStringLower(SHA256.HashData(original)) !=
            "10dc07de16da355b96154cb11550cae7146a0124730e87c71466e84fd63fb33c")
            throw new InvalidOperationException("Unsupported FFXIV resource assembly for Linux compatibility");
        using var pe = new PEReader(new MemoryStream(original, false));
        var metadata = pe.GetMetadataReader();
        var method = metadata.GetMethodDefinition(MetadataTokens.MethodDefinitionHandle(0x68));
        var declaring = metadata.GetTypeDefinition(method.GetDeclaringType());
        var body = pe.GetMethodBody(method.RelativeVirtualAddress);
        var il = body.GetILBytes()!;
        if (metadata.GetGuid(metadata.GetModuleDefinition().Mvid) != new Guid("3df23e26-6e4d-451f-a603-4ef197526e7b") ||
            metadata.GetString(declaring.Namespace) != "FFXIV_ACT_Plugin.Resource" || metadata.GetString(declaring.Name) != "ResourceFile" ||
            metadata.GetString(method.Name) != "LoadResourceDictionary" || method.RelativeVirtualAddress != 11616 ||
            !metadata.GetBlobBytes(method.Signature).AsSpan().SequenceEqual(new byte[] { 0x20, 0, 0x15, 0x12, 0x4d, 2, 9, 0x0e }) ||
            body.MaxStack != 6 || !body.LocalVariablesInitialized || MetadataTokens.GetToken((EntityHandle)body.LocalSignature) != 0x1100000e ||
            il.Length != 943 || Convert.ToHexStringLower(SHA256.HashData(il)) != "28a2a428fef07111bc6443127640f13c2d74813985c83701d299f3f27ae87ce9")
            throw new InvalidOperationException("FFXIV resource reader IL does not match the inspected build");
        var expectedRegions = new[]
        {
            (ExceptionRegionKind.Catch, 29, 159, 188, 55, 0x0100001d),
            (ExceptionRegionKind.Finally, 685, 42, 727, 14, 0),
            (ExceptionRegionKind.Finally, 352, 402, 754, 10, 0),
            (ExceptionRegionKind.Catch, 244, 522, 766, 57, 0x0100001d),
            (ExceptionRegionKind.Finally, 867, 62, 929, 12, 0)
        };
        if (!body.ExceptionRegions.Select(region => (region.Kind, region.TryOffset, region.TryLength,
            region.HandlerOffset, region.HandlerLength, region.CatchType.IsNil ? 0 : MetadataTokens.GetToken(region.CatchType)))
            .SequenceEqual(expectedRegions))
            throw new InvalidOperationException("FFXIV resource reader exception regions do not match the inspected build");
        var concat = metadata.GetMemberReference(MetadataTokens.MemberReferenceHandle(0x21));
        var concatType = metadata.GetTypeReference((TypeReferenceHandle)concat.Parent);
        var singleton = metadata.GetFieldDefinition(MetadataTokens.FieldDefinitionHandle(0x7f));
        var selector = metadata.GetMethodDefinition(MetadataTokens.MethodDefinitionHandle(0x145));
        if (metadata.GetUserString(MetadataTokens.UserStringHandle(0x445)) != "\r" ||
            metadata.GetUserString(MetadataTokens.UserStringHandle(0x449)) != "\n" ||
            metadata.GetString(concatType.Namespace) != "System" || metadata.GetString(concatType.Name) != "String" ||
            metadata.GetString(concat.Name) != "Concat" ||
            !metadata.GetBlobBytes(concat.Signature).AsSpan().SequenceEqual(new byte[] { 0, 2, 0x0e, 0x0e, 0x0e }) ||
            metadata.GetString(singleton.Name) != "<>9" || singleton.GetDeclaringType() != selector.GetDeclaringType() ||
            metadata.GetString(selector.Name) != "<LoadResourceDictionary>b__8_0")
            throw new InvalidOperationException("FFXIV resource delimiter/selector tokens do not match the inspected build");
        // Keep the complete fat header, all exception regions and every original instruction at IL86 or later intact.
        const int ilOffset = 3948;
        if (!original.AsSpan(ilOffset, il.Length).SequenceEqual(il) ||
            !original.AsSpan(ilOffset - 12, 12).SequenceEqual(new byte[] { 0x1b, 0x30, 6, 0, 0xaf, 3, 0, 0, 0x0e, 0, 0, 0x11 }))
            throw new InvalidOperationException("FFXIV resource reader file offset/header does not match the inspected build");
        byte[] replacement = [
            0x72, 0x45, 0x04, 0, 0x70, // ldstr CR
            0x72, 0x49, 0x04, 0, 0x70, // ldstr LF
            0x28, 0x21, 0, 0, 0x0a, // String.Concat(string,string) gives the original Windows CRLF delimiter.
            0xa2, 0x17, 0x6f, 0x57, 0, 0, 0x0a, // Preserve the array store, RemoveEmptyEntries and Split call.
            0x7e, 0x7f, 0, 0, 0x04, // Reuse the original singleton target.
            0xfe, 0x06, 0x45, 0x01, 0, 0x06,
            0x73, 0x58, 0, 0, 0x0a,
            0, 0, 0, 0, 0];
        var rewritten = (byte[])original.Clone();
        // Construct the same selector each read: removing its private cache frees room without moving any branch/EH boundary.
        replacement.CopyTo(rewritten, ilOffset + 43);
        return rewritten;
    }
}
