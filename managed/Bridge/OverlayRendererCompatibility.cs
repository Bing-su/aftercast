using System.Reflection;
using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;
using System.Runtime.Loader;
using System.Security.Cryptography;
using System.Text;

namespace Aftercast.Bridge;

internal static class OverlayRendererCompatibility
{
    private const string KnownHash = "7d5a9b39940e8a4470398707801fbf4f7d643e082e06dd9e244fbc80549d742f";
    internal static byte[] Rewrite(byte[] original)
    {
        if (Convert.ToHexStringLower(SHA256.HashData(original)) != KnownHash)
            throw new InvalidOperationException("Unsupported HtmlRenderer artifact for CopyMemory compatibility");
        using var pe = new PEReader(new MemoryStream(original, false));
        var metadata = pe.GetMetadataReader();
        var type = metadata.TypeDefinitions.Select(metadata.GetTypeDefinition).Single(type =>
            metadata.GetString(type.Namespace) == "RainbowMage.HtmlRenderer" && metadata.GetString(type.Name) == "NativeMethods");
        var method = type.GetMethods().Select(metadata.GetMethodDefinition).Single(method => metadata.GetString(method.Name) == "CopyMemory");
        var import = method.GetImport();
        if (metadata.GetString(import.Name) != "CopyMemory" || metadata.GetString(metadata.GetModuleReference(import.Module).Name) != "kernel32" ||
            !metadata.GetBlobBytes(method.Signature).AsSpan().SequenceEqual(new byte[] { 0, 3, 1, 0x18, 0x18, 9 }))
            throw new InvalidOperationException("Inspected CopyMemory import signature changed");
        var moduleName = metadata.GetModuleReference(import.Module).Name;
        if (metadata.MethodDefinitions.Select(metadata.GetMethodDefinition).Count(candidate =>
            !candidate.GetImport().Module.IsNil && candidate.GetImport().Module == import.Module) != 1)
            throw new InvalidOperationException("CopyMemory module is shared by other imports");
        var rewritten = (byte[])original.Clone();
        var heap = pe.PEHeaders.MetadataStartOffset + metadata.GetHeapMetadataOffset(HeapIndex.String);
        Replace(moduleName, "kernel32", "ucrtbase");
        // The internal method/import share a string. Calls within this pinned DLL use its MethodDef token, not the name.
        // Windows x64's unified calling convention accepts memcpy's ignored pointer return and zero-extended uint length.
        Replace(import.Name, "CopyMemory", "memcpy");
        return rewritten;
        void Replace(StringHandle handle, string expected, string replacement)
        {
            var offset = heap + MetadataTokens.GetHeapOffset(handle);
            var expectedBytes = Encoding.ASCII.GetBytes(expected + "\0");
            if (!original.AsSpan(offset, expectedBytes.Length).SequenceEqual(expectedBytes)) throw new InvalidOperationException("Inspected string heap changed");
            rewritten.AsSpan(offset, expectedBytes.Length).Clear();
            Encoding.ASCII.GetBytes(replacement, rewritten.AsSpan(offset));
        }
    }
    internal static Assembly? LoadIfKnown(AssemblyLoadContext context, string path)
    {
        if (!OperatingSystem.IsWindows() || IntPtr.Size != 8) return null;
        var bytes = File.ReadAllBytes(path);
        if (Convert.ToHexStringLower(SHA256.HashData(bytes)) != KnownHash) return null;
        using var image = new MemoryStream(Rewrite(bytes), false);
        return context.LoadFromStream(image);
    }
}
