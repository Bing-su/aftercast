open System
open System.IO
open System.Reflection
open System.Reflection.Metadata
open System.Reflection.Metadata.Ecma335
open System.Reflection.PortableExecutable

let publishFields path =
    // F# emits internal static fields with @ suffixes; expose only our two original ACT CLI fields.
    let bytes = File.ReadAllBytes(path)
    use stream = new MemoryStream(bytes, false)
    use pe = new PEReader(stream)
    let metadata = pe.GetMetadataReader()
    let types = metadata.TypeDefinitions |> Seq.map (fun handle -> metadata.GetTypeDefinition(handle)) |> Seq.toArray
    let named namespaceName typeName (definition: TypeDefinition) =
        metadata.GetString(definition.Namespace) = namespaceName && metadata.GetString(definition.Name) = typeName
    if not (types |> Array.exists (named "Bxivact.ActCompat" "HostEnvironment")) then
        failwith "Refusing to patch an assembly without our compatibility implementation"
    let globals = types |> Array.find (named "Advanced_Combat_Tracker" "ActGlobals")
    let fields = globals.GetFields() |> Seq.map (fun handle -> handle, metadata.GetFieldDefinition(handle)) |> Seq.toArray
    if fields.Length <> 2 then failwith "Unexpected ACT globals layout"
    for required in [ "oFormActMain"; "charName" ] do
        let handle, field = fields |> Array.find (fun (_, field) ->
            let name = metadata.GetString(field.Name)
            name = required || name = required + "@")
        if not (field.Attributes.HasFlag(FieldAttributes.Static)) then failwith "ACT global must be static"
        let access = field.Attributes &&& FieldAttributes.FieldAccessMask
        if access <> FieldAttributes.Assembly && access <> FieldAttributes.Public then failwith "Unexpected ACT field access flags"
        let row = MetadataTokens.GetRowNumber(handle)
        let offset = pe.PEHeaders.MetadataStartOffset + metadata.GetTableMetadataOffset(TableIndex.Field)
                     + (row - 1) * metadata.GetTableRowSize(TableIndex.Field)
        let flags = uint16 ((field.Attributes &&& ~~~FieldAttributes.FieldAccessMask) ||| FieldAttributes.Public)
        System.Buffers.Binary.BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(offset, 2), flags)
        if metadata.GetString(field.Name) = required + "@" then
            let suffix = pe.PEHeaders.MetadataStartOffset + metadata.GetHeapMetadataOffset(HeapIndex.String)
                         + MetadataTokens.GetHeapOffset(field.Name) + required.Length
            if bytes[suffix] <> byte '@' then failwith "Unexpected field-name heap layout"
            bytes[suffix] <- 0uy
    File.WriteAllBytes(path, bytes)
    // Re-read metadata without loading code: validate both names, access and static-ness after editing.
    use verifiedStream = File.OpenRead(path)
    use verifiedPe = new PEReader(verifiedStream)
    let verified = verifiedPe.GetMetadataReader()
    let names = fields |> Array.map (fun (handle, _) ->
        let field = verified.GetFieldDefinition(handle)
        if field.Attributes <> (FieldAttributes.Public ||| FieldAttributes.Static) then
            failwith "Published ACT global has unexpected flags"
        verified.GetString(field.Name)) |> Set.ofArray
    if names <> Set.ofList [ "oFormActMain"; "charName" ] then failwith "Published ACT field names differ"
    printfn "Published two original ACT static field contracts without adding C# or a runtime build step."

publishFields fsi.CommandLineArgs[1]
