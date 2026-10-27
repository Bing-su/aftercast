using System.Reflection.Emit;
using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;

// Read pinned original IL only: no native UI, clipboard, file exporter or external service is executed.
internal static class ReportExportPolicy
{
    internal static object Run(string path)
    {
        using var stream=File.OpenRead(path);using var pe=new PEReader(stream);var reader=pe.GetMetadataReader();
        var type=reader.TypeDefinitions.Single(handle=>reader.GetString(reader.GetTypeDefinition(handle).Name)=="FormActMain");
        byte[] Body(string name,params string[] parameters)
        {
            var method=reader.GetTypeDefinition(type).GetMethods().Select(reader.GetMethodDefinition).Single(method=>
                reader.GetString(method.Name)==name&&method.GetParameters().Select(parameter=>reader.GetString(reader.GetParameter(parameter).Name)).SequenceEqual(parameters));
            return pe.GetMethodBody(method.RelativeVirtualAddress).GetILBytes()!;
        }
        string Name(byte[] body,int offset)
        {
            var token=MetadataTokens.EntityHandle(BitConverter.ToInt32(body,offset+1));
            return token.Kind switch {
                HandleKind.MethodDefinition=>reader.GetString(reader.GetMethodDefinition((MethodDefinitionHandle)token).Name),
                HandleKind.MemberReference=>reader.GetString(reader.GetMemberReference((MemberReferenceHandle)token).Name),
                _=>throw new InvalidDataException("Unexpected original report IL token")};
        }
        static void Check(bool value,string field){if(!value)throw new InvalidDataException("Original report policy changed: "+field);}
        void Call(byte[] body,int offset,string name){Check(body[offset]==OpCodes.Call.Value||body[offset]==OpCodes.Callvirt.Value||body[offset]==OpCodes.Newobj.Value,"call opcode");Check(Name(body,offset)==name,name);}
        var basic=Body("GetTextExport","Encounter","ExportFormatting");
        Check(basic[0x0003]==OpCodes.Ldnull.Value&&basic[0x0004]==OpCodes.Ldc_I4_0.Value,"unaligned default/font and max-lines fallback");Call(basic,0x0005,"GetTextExport");
        var core=Body("GetTextExport","RtfFormat","Encounter","ExportFormatting","TabulateFont","MaxLines");
        Check(core[0x000b]==OpCodes.Ldc_I4.Value&&BitConverter.ToInt32(core,0x000c)==256,"default LF line budget");
        Call(core,0x0065,".ctor");Call(core,0x006a,"Sort");Call(core,0x0071,"Reverse");
        Call(core,0x009e,"GetIsSelective");Call(core,0x00a6,"GetIgnoreEnemies");Call(core,0x00c2,"GetAllies");
        Check(core[0x02c6]==OpCodes.Ldstr.Value&&reader.GetUserString(MetadataTokens.UserStringHandle(BitConverter.ToInt32(core,0x02c7)&0xffffff))=="Unknown","Unknown actor exclusion");
        Check(core[0x06ee]==OpCodes.Ldc_I4_S.Value&&core[0x06ef]==10,"LF delimiter");Call(core,0x06f1,"Split");
        var table=Body("cmiEDExportXML_Click","sender","e");
        Check(table[0x0097]==OpCodes.Ldstr.Value&&reader.GetUserString(MetadataTokens.UserStringHandle(BitConverter.ToInt32(table,0x0098)&0xffffff))=="EncounterTable","table XML is distinct from encounter interchange XML");
        return new {text=new{unaligned=true,descendingSort=true,defaultLines=256,unknownActorExcluded=true,selectiveIgnoreEnemies="all actors",lfFraming=true},tableXmlRoot="EncounterTable",nativeUiExecuted=false,clipboardAccessExecuted=false,exportFileExecuted=false};
    }
}
