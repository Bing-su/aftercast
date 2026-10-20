open System
open System.IO
open System.Reflection.Metadata
open System.Reflection.PortableExecutable
open System.Security.Cryptography

let prepare () =
    // Bind the experiment to the reviewed artifact before trusting its public identity.
    let original = Path.GetFullPath(Path.Combine(__SOURCE_DIRECTORY__, "../../binaries/act/Advanced Combat Tracker.exe"))
    use input = File.OpenRead(original)
    let digest = Convert.ToHexString(SHA256.HashData(input)).ToLowerInvariant()
    if digest <> "934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657" then
        failwith "ACT reference hash differs from the reviewed 3.8.5.288 artifact"
    input.Position <- 0L
    use pe = new PEReader(input)
    let metadata = pe.GetMetadataReader()
    let assembly = metadata.GetAssemblyDefinition()
    let publicKey = metadata.GetBlobBytes(assembly.PublicKey)
    if metadata.GetString(assembly.Name) <> "Advanced Combat Tracker" || assembly.Version <> Version(3, 8, 5, 288) then
        failwith "Unexpected ACT assembly identity"
    let destination = Path.GetFullPath(Path.Combine(__SOURCE_DIRECTORY__, "../ActCompat/ACT.publickey"))
    if File.Exists(destination) then
        if File.ReadAllBytes(destination) <> publicKey then
            failwith "The checked-in ACT public key differs from the reviewed original"
    else File.WriteAllBytes(destination, publicKey)
    printfn "Verified ACT reference; extracted %d public-key bytes (no private key)." publicKey.Length

prepare ()
