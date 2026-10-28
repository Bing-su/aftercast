using System.Text.Json;
using Aftercast.ActCompat;

internal static class GraphNumericChecks
{
    internal static void Run()
    {
        using var fixture=typeof(GraphNumericChecks).Assembly.GetManifestResourceStream("GraphContracts.json") ?? throw new Exception("Missing graph fixture");
        using var json=JsonDocument.Parse(fixture);
        foreach(var item in json.RootElement.GetProperty("line").EnumerateArray())
        {
            var samples=item.GetProperty("actions").EnumerateArray().Select(pair=>new GraphNumeric.Sample(pair[0].GetDouble(),pair[1].GetInt64())).ToArray();
            var actual=GraphNumeric.Line(item.GetProperty("duration").GetInt32(),item.GetProperty("window").GetInt32(),item.GetProperty("rolling").GetBoolean(),samples);
            long[] Expected(string key)=>item.GetProperty(key).EnumerateArray().Select(value=>value.GetInt64()).ToArray();
            if(!actual.Raw.SequenceEqual(Expected("raw"))||!actual.Rates.SequenceEqual(Expected("integerRates"))||!actual.Plotted.SequenceEqual(Expected("plottedRates")))
                throw new Exception("Graph numeric fixture mismatch: "+item.GetProperty("name").GetString());
        }
        try{GraphNumeric.Line(4,2,false,[new(-10,1)]);throw new Exception("Expected invalid bucket");}catch(InvalidDataException){}
        try{GraphNumeric.Line(4,0,true,[]);throw new Exception("Expected invalid window");}catch(ArgumentOutOfRangeException){}
        static void Expect<T>(Action action) where T : Exception
        {
            try { action(); } catch (T) { return; }
            throw new Exception("Expected " + typeof(T).Name);
        }
        _ = GraphNumeric.Line(0, 1, true, []);
        _ = GraphNumeric.Line(360000, 120, false, []);
        Expect<ArgumentOutOfRangeException>(() => GraphNumeric.Line(360001, 1, true, []));
        Expect<ArgumentOutOfRangeException>(() => GraphNumeric.Line(1, 121, true, []));
        _ = GraphNumeric.Line(1, 1, true, Enumerable.Repeat(new GraphNumeric.Sample(0, 1), 16384));
        Expect<InvalidDataException>(() => GraphNumeric.Line(1, 1, true, Enumerable.Repeat(new GraphNumeric.Sample(0, 1), 16385)));
        Expect<InvalidDataException>(() => GraphNumeric.Line(1, 1, true, [new(double.NaN, 1)]));
        _ = GraphNumeric.Line(1, 1, true, [new(double.NaN, 0)]);
        var negative = GraphNumeric.Line(4, 2, false, [new(-1, 2), new(-2, 4)]);
        if (negative.Raw[1] != 2 || negative.Raw[0] != 4) throw new Exception("Negative bucket mismatch");
        var overflow = GraphNumeric.Line(1, 1, true, [new(0, long.MaxValue), new(0, 1)]);
        if (overflow.Raw[0] != long.MinValue) throw new Exception("Overflow mismatch");
        Console.WriteLine("Production graph line arithmetic matches explicit ACT numeric fixtures.");
    }
}
