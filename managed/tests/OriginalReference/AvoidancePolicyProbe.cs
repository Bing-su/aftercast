using System.Reflection.Emit;
using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;
using System.Text.Json;

// Pin report branches without executing native ListView/form code.
internal static class AvoidancePolicyProbe
{
    internal static int Run(string path, string fixture)
    {
        using var input = File.OpenRead(path); using var pe = new PEReader(input); var reader = pe.GetMetadataReader();
        var type = reader.TypeDefinitions.Single(h => reader.GetString(reader.GetTypeDefinition(h).Name) == "FormAvoidanceReport");
        byte[] Body(string name) => pe.GetMethodBody(reader.GetTypeDefinition(type).GetMethods().Select(reader.GetMethodDefinition)
            .Single(m => reader.GetString(m.Name) == name).RelativeVirtualAddress).GetILBytes()!;
        string Name(byte[] code, int offset)
        {
            var h = MetadataTokens.EntityHandle(BitConverter.ToInt32(code, offset + 1));
            return h.Kind switch {
                HandleKind.MethodDefinition => reader.GetString(reader.GetMethodDefinition((MethodDefinitionHandle)h).Name),
                HandleKind.MemberReference => reader.GetString(reader.GetMemberReference((MemberReferenceHandle)h).Name),
                HandleKind.FieldDefinition => reader.GetString(reader.GetFieldDefinition((FieldDefinitionHandle)h).Name),
                _ => throw new InvalidDataException("Unexpected avoidance token") };
        }
        void Check(bool value, string detail) { if (!value) throw new InvalidDataException("Original avoidance policy: " + detail); }
        void Token(byte[] b, int i, string name) => Check(Name(b,i) == name, name);
        var actor = Body("ShowAvoidanceReport");
        Token(actor,0x40,"DamageSwingTypes"); Token(actor,0x57,"SwingTypeToDamageTypeDataLinksOutgoing"); Token(actor,0xc0,"get_AllInc");
        Token(actor,0x175,"op_Equality"); Token(actor,0x1b9,"AddCombatAction"); Token(actor,0x1cf,"AddCombatAction");
        Check(actor[0x404] == OpCodes.Stsfld.Value && actor[0x403] == OpCodes.Ldc_I4_0.Value,"force block false");
        Token(actor,0x43a,"get_Swings"); Token(actor,0x495,"get_Hits");
        Check(actor[0x49a] == OpCodes.Sub.Value && actor[0x4a9] == OpCodes.Div.Value && actor[0x4b6] == OpCodes.Div.Value,"non-hit and swing ratios");
        Token(actor,0x500,"get_Item"); Token(actor,0x505,"DamageTypeDataNonSkillDamage"); Token(actor,0x50a,"Contains");
        Token(actor,0x58d,"op_Equality");
        Check(actor[0x599] == OpCodes.Ldloc_S.Value && actor[0x59a] == 39,"filter checks failed action, not candidate");
        Token(actor,0x59b,"get_SwingType"); Token(actor,0x5d3,"get_Average"); Token(actor,0x5d8,"IsNaN"); Token(actor,0x5ec,"get_Average");
        var encounter = Body("ShowEncounterAvoidance");
        Token(encounter,0x150,"get_AllInc"); Token(encounter,0x186,"op_Equality"); Token(encounter,0x1c9,"op_Inequality");
        Token(encounter,0x253,"GetAllies");
        Check(reader.GetUserString(MetadataTokens.UserStringHandle(BitConverter.ToInt32(encounter,0x25d)&0xffffff)) == "YOU","literal YOU alias");
        Token(encounter,0x298,"Contains"); Token(encounter,0x36a,"Contains");
        Check(encounter[0x2ec] == OpCodes.Br_S.Value && 0x2ee + (sbyte)encounter[0x2ed] == 0x2ff,"first matching alias activates group");
        using var json = JsonDocument.Parse(File.ReadAllText(fixture)); var data=json.RootElement;
        var allies=data.GetProperty("allies").EnumerateArray().Select(x=>x.GetString()!).ToArray();
        var labels=data.GetProperty("labels").EnumerateArray().Select(x=>x.GetString()!).ToArray();
        // These transformed loops are guarded by the IL above; they are not native UI execution.
        var active=new List<string>();
        foreach(var label in labels) foreach(var ally in allies) if(label.Contains(ally)) { if(!active.Contains(ally))active.Add(ally);break; }
        Check(active.Count==data.GetProperty("expectedGroups").EnumerateObject().Count(),"active alias groups");
        foreach(var expected in data.GetProperty("expectedGroups").EnumerateObject())
            Check(active.Contains(expected.Name) && labels.Where(label=>label.Contains(expected.Name)).Order().SequenceEqual(expected.Value.EnumerateArray().Select(x=>x.GetString()!).Order()),"ordinal alias filter");
        var routed=data.GetProperty("links").EnumerateArray().GroupBy(x=>x.GetString()!).ToDictionary(g=>g.Key,g=>g.Count());
        foreach(var expected in data.GetProperty("expectedRouting").EnumerateObject())Check(routed[expected.Name]==expected.Value.GetInt32(),"duplicate links remain duplicate");
        Console.WriteLine(JsonSerializer.Serialize(new { originalAvoidanceIL = "passed", transformedPolicyFixture = "passed" }));
        return 0;
    }
}
