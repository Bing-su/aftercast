using System.Reflection.Emit;
using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;

// Inspect the pinned original's actual branch; never construct FormActMain or run its native import thread.
internal static class ImportPolicy
{
    internal static object Run(string path)
    {
        using var stream=File.OpenRead(path);using var pe=new PEReader(stream);var reader=pe.GetMetadataReader();
        byte[] Body(string typeName,string methodName,int parameters)
        {
            var type=reader.TypeDefinitions.Single(handle=>reader.GetString(reader.GetTypeDefinition(handle).Name)==typeName);
            var method=reader.GetTypeDefinition(type).GetMethods().Select(reader.GetMethodDefinition)
                .Single(method=>reader.GetString(method.Name)==methodName&&method.GetParameters().Count==parameters);
            return pe.GetMethodBody(method.RelativeVirtualAddress).GetILBytes()!;
        }
        string Name(byte[] body,int offset)
        {
            var token=MetadataTokens.EntityHandle(BitConverter.ToInt32(body,offset+1));
            return token.Kind switch{
                HandleKind.MethodDefinition=>reader.GetString(reader.GetMethodDefinition((MethodDefinitionHandle)token).Name),
                HandleKind.MemberReference=>reader.GetString(reader.GetMemberReference((MemberReferenceHandle)token).Name),
                HandleKind.FieldDefinition=>reader.GetString(reader.GetFieldDefinition((FieldDefinitionHandle)token).Name),
                _=>throw new InvalidDataException("Unexpected original IL token kind")};
        }
        static void Check(bool value,string description){if(!value)throw new InvalidDataException("Original import branch changed: "+description);}
        void Call(byte[] body,int offset,string name){Check(body[offset]==OpCodes.Call.Value||body[offset]==OpCodes.Callvirt.Value,"call opcode");Check(Name(body,offset)==name,name);}
        static void Branch(byte[] body,int offset,OpCode opcode,int target){Check(body[offset]==opcode.Value&&offset+2+(sbyte)body[offset+1]==target,"conditional branch");}
        var import=Body("FormActMain","ImportFile",4);
        // time >= begin && (time <= end || (extend && InCombat)), decided before ParseRawLogLine.
        Call(import,0x0a14,"op_GreaterThanOrEqual");Branch(import,0x0a19,OpCodes.Brfalse_S,0x0a58);
        Call(import,0x0a1f,"op_LessThanOrEqual");Branch(import,0x0a24,OpCodes.Brtrue_S,0x0a31);
        Check(import[0x0a26]==OpCodes.Ldarg_2.Value,"extend argument");Branch(import,0x0a27,OpCodes.Brfalse_S,0x0a58);
        Call(import,0x0a2a,"get_InCombat");Branch(import,0x0a2f,OpCodes.Brfalse_S,0x0a58);Call(import,0x0a51,"ParseRawLogLine");
        Call(import,0x0a5c,"op_GreaterThan");Branch(import,0x0a61,OpCodes.Brfalse_S,0x0a87);
        Check(import[0x0a63]==OpCodes.Leave.Value&&0x0a68+BitConverter.ToInt32(import,0x0a64)==0x0b32,"first rejected later row terminates");
        Check(Name(import,0x01f5)=="rbImportFromBegining"&&Name(import,0x0201)=="MinValue","file begin sentinel");
        Check(Name(import,0x020e)=="rbImportToEnd"&&Name(import,0x022c)=="MaxValue","idle file end sentinel");
        Call(import,0x021b,"get_InCombat");Call(import,0x0223,"get_LastKnownTime");
        var controls=Body("IO_ImportLog","InitializeComponent",0);
        foreach(var (field,load,call)in new[]{(0x024d,0x0252,0x0253),(0x0422,0x0427,0x0428),(0x0c49,0x0c4e,0x0c4f)})
        {Check(controls[load]==OpCodes.Ldc_I4_1.Value,"checked UI default: "+Name(controls,field));Call(controls,call,"set_Checked");}
        var dispatch=Body("FormActMain","ParseRawLogLine",4);Call(dispatch,0x0014,"get_InCombat");Call(dispatch,0x001e,"CheckIdleEndCombat");
        var clip=Body("FormActMain","ImportClip",0);
        Call(clip,0x0192,"ContainsText");Call(clip,0x0199,"GetText");
        Check(clip[0x0231]==OpCodes.Newobj.Value&&Name(clip,0x0231)==".ctor","clipboard string reader");
        var constructor=reader.GetMemberReference((MemberReferenceHandle)MetadataTokens.EntityHandle(BitConverter.ToInt32(clip,0x0232)));
        Check(reader.GetString(reader.GetTypeReference((TypeReferenceHandle)constructor.Parent).Name)=="StringReader","Unicode text reader");
        Call(clip,0x027a,"ReadLine");Call(clip,0x0283,"IsNullOrEmpty");
        Check(clip[0x0348]==OpCodes.Ldc_I4.Value&&BitConverter.ToInt32(clip,0x0349)==1000,"clipboard empty limit");
        Branch(clip,0x034d,OpCodes.Ble_S,0x0354);
        Call(clip,0x02d6,"set_LastKnownTime");Call(clip,0x032d,"get_LastKnownTime");Call(clip,0x0339,"ParseRawLogLine");
        return new{policy="time >= begin && (time <= end || extend && InCombat before ParseRawLogLine)",fileBeginDefault=true,fileEndDefault=true,extendDefault=true,
            idleAdmissionFileEnd="DateTime.MaxValue",activeLiveFileEnd="LastKnownTime (aftercast requires idle admission)",clipboard=new{input="Unicode StringReader",consecutiveEmptyLimit=1000,undated="LastKnownTime",clipboardAccessExecuted=false},nativeImportExecuted=false};
    }
}
