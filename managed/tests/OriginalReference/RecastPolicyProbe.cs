using System.Reflection.Emit;
using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;
using System.Text.Json;

// Pins native report IL, then exercises its transformed selection/interval loops without WinForms.
internal static class RecastPolicyProbe
{
    internal static int Run(string path, string fixture)
    {
        using var input = File.OpenRead(path); using var pe = new PEReader(input); var reader = pe.GetMetadataReader();
        var type = reader.TypeDefinitions.Single(h => reader.GetString(reader.GetTypeDefinition(h).Name) == "FormSpellRecastCalc");
        byte[] Body(string name) => pe.GetMethodBody(reader.GetTypeDefinition(type).GetMethods().Select(reader.GetMethodDefinition)
            .Single(method => reader.GetString(method.Name) == name).RelativeVirtualAddress).GetILBytes()!;
        void Check(bool value) { if (!value) throw new InvalidDataException("Original recast policy changed"); }
        string Name(byte[] code, int offset)
        {
            var handle = MetadataTokens.EntityHandle(BitConverter.ToInt32(code, offset + 1));
            return handle.Kind switch {
                HandleKind.MethodDefinition => reader.GetString(reader.GetMethodDefinition((MethodDefinitionHandle)handle).Name),
                HandleKind.MemberReference => reader.GetString(reader.GetMemberReference((MemberReferenceHandle)handle).Name),
                _ => throw new InvalidDataException("Unexpected recast token") };
        }
        string Literal(byte[] code, int offset) => reader.GetUserString(MetadataTokens.UserStringHandle(BitConverter.ToInt32(code, offset + 1) & 0xffffff));
        var show = Body("ShowMedian"); var calc = Body("RecalculateDelays");
        Check(show[0x2e1] == OpCodes.Ldc_I4_S.Value && show[0x2e2] == 11 && show[0x2f9] == OpCodes.Ldc_I4_1.Value);
        Check(calc[0x17f] == OpCodes.Ldc_I4.Value && BitConverter.ToInt32(calc, 0x180) == 12345);
        Check(calc[0x1d9] == OpCodes.Conv_I4.Value && calc[0x38c] == OpCodes.Conv_R4.Value && calc[0x39c] == OpCodes.Div.Value && calc[0x39d] == OpCodes.Conv_I4.Value);
        Check(calc[0x385] == OpCodes.Ldc_I4_S.Value && calc[0x386] == 50 && calc[0x3bc] == OpCodes.Ldc_I4_S.Value && calc[0x3bd] == 121);
        Check(Name(calc, 0x200) == "get_Damage" && calc[0x20a] == OpCodes.Ldc_I4_M1.Value && Name(calc, 0x225) == "op_GreaterThan");
        Check(calc[0x247] == OpCodes.Ldc_R8.Value && BitConverter.ToDouble(calc, 0x248) == 31 && Name(calc, 0x250) == "AddSeconds" && Name(calc, 0x255) == "op_LessThan");
        Check(Name(calc, 0x284) == "GetCombatant" && Name(calc, 0x2f3) == "get_Death" && Name(calc, 0x2f8) == "op_Equality");
        Check(Name(calc, 0x324) == "op_GreaterThanOrEqual" && Name(calc, 0x340) == "op_LessThan");
        string[] names = ["Traumatic Swipe", "Traumatischer Greifer", "Coup traumatisant", "トラウマティック・スワイプ", "Травмирующий удар силача"];
        int[] offsets = [0x40, 0x75, 0xaa, 0xdc, 0x10e];
        for (var i = 0; i < names.Length; i++) Check(Literal(calc, offsets[i]) == names[i]);
        Check((int)TimeSpan.FromTicks(10_000_000_999_999_999).TotalSeconds == 1_000_000_100);
        using var json = JsonDocument.Parse(File.ReadAllText(fixture));
        foreach (var item in json.RootElement.GetProperty("cases").EnumerateArray())
        {
            var seconds = item.GetProperty("seconds").EnumerateArray().Select(value => value.GetInt32()).ToArray();
            var mods = item.GetProperty("modified").EnumerateArray().Select(value => value.ValueKind == JsonValueKind.Null ? (bool?)null : value.GetBoolean()).ToArray();
            var restricted = item.GetProperty("restrict").GetBoolean(); var useModifier = item.GetProperty("useModifier").GetBoolean();
            var initial = seconds.Select((value, index) => index == 0 || value - seconds[index - 1] > (restricted ? 11 : 1)).ToArray();
            Check(initial.SequenceEqual(item.GetProperty("initial").EnumerateArray().Select(value => value.GetBoolean())));
            var selected = item.GetProperty("selected").EnumerateArray().Select(value => value.GetBoolean()).ToArray();
            var indexes = Enumerable.Range(0, seconds.Length).Where(index => selected[index]).ToArray(); var delays = new List<int>();
            for (var i = 1; i < indexes.Length; i++)
            {
                var start = indexes[i-1]; var delta = seconds[indexes[i]] - seconds[start];
                if (useModifier && mods[start] is { } enabled) delta = (int)((float)delta / (enabled ? 1.5f : 1f));
                delays.Add(delta);
            }
            Check(delays.SequenceEqual(item.GetProperty("intervals").EnumerateArray().Select(value => value.GetInt32())));
            var min = delays.Where(value => value > 1 && value < 12345).DefaultIfEmpty(12345).Min();
            var expectedMin = item.GetProperty("minimum"); Check(expectedMin.ValueKind == JsonValueKind.Null ? min == 12345 : min == expectedMin.GetInt32());
            var bins = delays.Where(value => value > 1 && (!restricted || value < 121)).GroupBy(value => value).OrderBy(group => group.Key).Select(group => (group.Key, group.Count())).ToArray();
            var expected = item.GetProperty("histogram").EnumerateArray().Select(value => (value[0].GetInt32(), value[1].GetInt32())).ToArray();
            Check(bins.SequenceEqual(expected));
        }
        Console.WriteLine("Original recast IL and transformed interval fixture passed (native form not executed).");
        return 0;
    }
}
