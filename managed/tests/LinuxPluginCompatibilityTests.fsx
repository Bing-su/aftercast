#load "../Bridge/LinuxPluginCompatibility.fs"

open System
open System.IO
open System.Reflection.Metadata
open System.Reflection.Metadata.Ecma335
open System.Reflection.PortableExecutable
open System.Security.Cryptography
open Bxivact.Bridge

let check condition = if not condition then failwith "Linux plugin compatibility check failed"
let path = "binaries/FFXIV_ACT_Plugin_SDK_3.0.3.3/SDK\\FFXIV_ACT_Plugin.Memory.dll"
let original = File.ReadAllBytes(path)
let before = SHA256.HashData(original)
let rewritten = LinuxPluginCompatibility.rewriteMemory original
check (SHA256.HashData(original) = before)
check (SHA256.HashData(File.ReadAllBytes(path)) = before)
check (rewritten.Length = original.Length)
check (not (Object.ReferenceEquals(original, rewritten)))

// Verify the preserved metadata/header and exact IL bounds independently of the rewrite's offsets.
let verifyIl () =
    use stream = new MemoryStream(rewritten, false)
    use pe = new PEReader(stream)
    let metadata = pe.GetMetadataReader()
    check (metadata.GetGuid(metadata.GetModuleDefinition().Mvid) = Guid("89b13c66-df57-43bf-8217-c84f5c77f5f6"))
    let method = metadata.GetMethodDefinition(MetadataTokens.MethodDefinitionHandle(0x96))
    check (metadata.GetString(method.Name) = "SetIsAdministratorFlag")
    let il = pe.GetMethodBody(method.RelativeVirtualAddress).GetILBytes()
    check (Convert.ToHexString(il[0..5]) = "02288E000006")
    check (il[6] = 0x16uy)
    check (il[7..25] |> Array.forall ((=) 0uy))
    check (Convert.ToHexString(il[26..31]) = "6F820000062A")
verifyIl ()
for index in 0 .. original.Length - 1 do
    if index < 9743 || index > 9762 then check (original[index] = rewritten[index])

let rejected bytes =
    try
        LinuxPluginCompatibility.rewriteMemory bytes |> ignore
        failwith "Unrecognized assembly was rewritten"
    with :? InvalidOperationException as error ->
        check (error.Message.Contains("Unsupported FFXIV memory assembly"))
rejected [||]
rejected rewritten
let changed = Array.copy original
changed[100] <- changed[100] ^^^ 1uy
rejected changed
check (not (LinuxPluginCompatibility.isKnownPlugin (Array.zeroCreate 32)))
let main = File.ReadAllBytes("binaries/FFXIV_ACT_Plugin_SDK_3.0.3.3/FFXIV_ACT_Plugin.dll")
check (LinuxPluginCompatibility.isKnownPlugin (SHA256.HashData(main)) = OperatingSystem.IsLinux())
printfn "Pinned Linux administrator-check rewrite, byte preservation and unknown-build rejection passed."
