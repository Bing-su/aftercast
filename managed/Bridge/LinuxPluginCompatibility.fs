namespace Bxivact.Bridge

open System
open System.IO
open System.IO.Compression
open System.Reflection
open System.Runtime.Loader
open System.Security.Cryptography

/// Keep the Linux adjustment tied to inspected bytes; never rewrite an unknown plugin build.
module internal LinuxPluginCompatibility =
    let isKnownPlugin hash =
        OperatingSystem.IsLinux()
        && Convert.ToHexStringLower(hash: byte[]) = "af1031cdba8eeba8f53dd9d618e58d62d4919bfa77db9b31c568bd04cb359093"

    let rewriteMemory (original: byte[]) =
        if original.Length <> 70144
           || Convert.ToHexStringLower(SHA256.HashData(original)) <> "0363753efe0d7ae52c0de73653a5436cf83cf655ca47515ce7aa33908f82d897" then
            invalidOp "Unsupported FFXIV memory assembly for Linux compatibility"
        // REA: method 0x06000096, tiny header at 9736, 32 IL bytes; retain get_Current and the setter.
        let ilOffset = 9737
        let il = original.AsSpan(ilOffset, 32)
        if Convert.ToHexStringLower(SHA256.HashData(il)) <> "7879a598366bd64b02ddf60262f86130d8e4b73fd8990a8293417b9e467129e1" then
            invalidOp "FFXIV administrator-check IL does not match the inspected build"
        let rewritten = Array.copy original
        // Linux supplies no Windows administrator token: report false, preserving all surrounding IL.
        Array.Fill(rewritten, 0uy, ilOffset + 6, 20)
        rewritten[ilOffset + 6] <- 0x16uy // ldc.i4.0, followed by nineteen nops before set_IsAdministrator.
        rewritten

    let loadMemory (context: AssemblyLoadContext) (plugin: Assembly) =
        use resource = plugin.GetManifestResourceStream("costura.ffxiv_act_plugin.memory.dll.compressed")
        if isNull resource then invalidOp "Pinned FFXIV plugin has no embedded memory assembly"
        use compressed = new DeflateStream(resource, CompressionMode.Decompress)
        // Bound decompression before hashing; the supported resource has one fixed, inspected size.
        let bytes = Array.zeroCreate<byte> 70144
        compressed.ReadExactly(bytes)
        if compressed.ReadByte() <> -1 then invalidOp "Embedded FFXIV memory assembly exceeds its pinned size"
        use image = new MemoryStream(rewriteMemory bytes, false)
        context.LoadFromStream(image)
