using System.Reflection;
using System.Reflection.Emit;
using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;
using System.Text.Json;

// Pins the original IL before exercising a transformed numeric loop without native UI.
internal static class TimelinePolicyProbe
{
    internal static int Run(Assembly act, string path, string fixture)
    {
        using var input = File.OpenRead(path); using var pe = new PEReader(input); var reader = pe.GetMetadataReader();
        var type = reader.TypeDefinitions.Single(h => reader.GetString(reader.GetTypeDefinition(h).Name) == "FormTimeLine");
        byte[] Body(string name) => pe.GetMethodBody(reader.GetTypeDefinition(type).GetMethods().Select(reader.GetMethodDefinition)
            .Single(method => reader.GetString(method.Name) == name).RelativeVirtualAddress).GetILBytes()!;
        string Name(byte[] code, int offset)
        {
            var h = MetadataTokens.EntityHandle(BitConverter.ToInt32(code, offset + 1));
            return h.Kind switch {
                HandleKind.MethodDefinition => reader.GetString(reader.GetMethodDefinition((MethodDefinitionHandle)h).Name),
                HandleKind.MemberReference => reader.GetString(reader.GetMemberReference((MemberReferenceHandle)h).Name),
                HandleKind.FieldDefinition => reader.GetString(reader.GetFieldDefinition((FieldDefinitionHandle)h).Name),
                _ => throw new InvalidDataException("Unexpected timeline token") };
        }
        void Check(bool value) { if (!value) throw new InvalidDataException("Original timeline policy changed"); }
        var body = Body("ShowTimeline");
        Check(Name(body, 0x953) == "DamageTypeDataOutgoingDamage" && Name(body, 0x99a) == "DamageTypeDataIncomingDamage");
        Check(Name(body, 0x9e1) == "DamageTypeDataOutgoingHealing" && Name(body, 0xa25) == "DamageTypeDataIncomingHealing");
        Check(Name(body, 0xa5b) == "get_Damage" && Name(body, 0xa60) == "op_Implicit" && body[0xa67] == OpCodes.Ble.Value);
        Check(Name(body, 0xa79) == "get_StartTime" && Name(body, 0xa87) == "get_TotalSeconds" && body[0xa8c] == OpCodes.Conv_I4.Value);
        Check(Name(body, 0xaa5) == "op_Implicit" && Name(body, 0xab1) == "op_Addition" && Name(body, 0xae7) == "op_Implicit" && Name(body, 0xaf3) == "op_Addition");
        Check(body[0xac5] == OpCodes.Blt_S.Value && body[0xacd] == OpCodes.Bge_S.Value && body[0xad2] == OpCodes.Blt_S.Value);
        Check(body[0xadd] == OpCodes.Div.Value && body[0xadf] == OpCodes.Add.Value);
        var dnum = act.GetType("Advanced_Combat_Tracker.Dnum", true)!;
        var add = dnum.GetMethod("op_Addition")!;
        var number = dnum.GetProperty("Number")!;
        long Sum(long left, long right)
        {
            var a = Activator.CreateInstance(dnum, new object[] { left < -10 ? -9L : left })!;
            var b = Activator.CreateInstance(dnum, new object[] { right })!;
            return (long)number.GetValue(add.Invoke(null, new[] { a, b }))!;
        }
        using var json = JsonDocument.Parse(File.ReadAllText(fixture));
        foreach (var item in json.RootElement.GetProperty("cases").EnumerateArray())
        {
            var duration = item.GetProperty("duration").GetInt32(); var window = item.GetProperty("window").GetInt32();
            var rolling = item.GetProperty("rolling").GetBoolean(); var points = rolling ? duration : duration / window;
            var bins = new long[points * 2];
            foreach (var row in item.GetProperty("samples").EnumerateArray())
            {
                var amount = row[1].GetInt64(); if (amount <= 0) continue;
                var offset = (int)row[0].GetDouble();
                if (rolling) { for (var i = offset; offset >= 0 && i < offset + window && i < duration; i++) bins[i] = Sum(bins[i], amount); }
                else { var i = offset / window + 1; bins[i] = Sum(bins[i], amount); }
            }
            var expected = item.GetProperty("displayed").EnumerateArray().Select(value => value.GetInt64()).ToArray();
            Check(Enumerable.Range(1, points).Select(i => bins[i] / window).SequenceEqual(expected));
        }
        Console.WriteLine("Original timeline IL and transformed fixture passed (native form not executed).");
        return 0;
    }
}
