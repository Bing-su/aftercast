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
        var dist = json.RootElement.GetProperty("distribution");
        var shares = GraphNumeric.Distribution(dist.GetProperty("allSql").GetDouble(), dist.GetProperty("attackSql").EnumerateArray().Select((v, i) => new GraphNumeric.CategoryValue(i.ToString(), v.GetDouble())));
        if (!shares.Select(x => x.Ratio).SequenceEqual(dist.GetProperty("includedRatios").EnumerateArray().Select(x => x.GetDouble()))) throw new Exception("Distribution fixture mismatch");
        Expect<InvalidDataException>(() => GraphNumeric.Distribution(0, []));
        Expect<InvalidDataException>(() => GraphNumeric.Distribution(-1, []));
        Expect<InvalidDataException>(() => GraphNumeric.Distribution(double.NaN, []));
        if (GraphNumeric.Distribution(1, []).Length != 0) throw new Exception("Empty distribution");
        var resist = json.RootElement.GetProperty("resist");
        var resistShares = GraphNumeric.Resist(resist.GetProperty("actions").EnumerateArray().Select(x => new GraphNumeric.ResistAction(x[0].GetString()!, x[1].GetInt64())), resist.GetProperty("wardPrefix").GetString()!, resist.GetProperty("allLabel").GetString()!);
        foreach (var expected in resist.GetProperty("ratios").EnumerateObject())
            if (resistShares.Single(x => x.Label == expected.Name).Ratio != expected.Value.GetDouble()) throw new Exception("Resist fixture mismatch");
        Expect<InvalidDataException>(() => GraphNumeric.Resist([], "Warded/", "All"));
        Expect<InvalidDataException>(() => GraphNumeric.Resist([new("Fire", long.MaxValue), new("Fire", 1)], "Warded/", "All"));
        var unicode = GraphNumeric.Resist([new("😀abcdeFire", 1)], "😀abcde", "All");
        if (unicode.Single().Label != "Fire") throw new Exception("UTF-16 prefix mismatch");
        _ = GraphNumeric.Distribution(1, Enumerable.Repeat(new GraphNumeric.CategoryValue("x", 1), 16384));
        Expect<InvalidDataException>(() => GraphNumeric.Distribution(1, Enumerable.Repeat(new GraphNumeric.CategoryValue("x", 1), 16385)));
        _ = GraphNumeric.Resist(Enumerable.Repeat(new GraphNumeric.ResistAction("x", 1), 16384), "Warded/", "All");
        Expect<InvalidDataException>(() => GraphNumeric.Resist(Enumerable.Repeat(new GraphNumeric.ResistAction("x", 1), 16385), "Warded/", "All"));
        var collision = GraphNumeric.Resist([new("All", 10), new("Fire", 10)], "Warded/", "All");
        if (collision.Single().Ratio != 1.0 / 3.0) throw new Exception("All collision mismatch");
        var zeroGroup = GraphNumeric.Resist([new("Zero", -1), new("Fire", 1)], "Warded/", "All");
        if (zeroGroup.Single(x => x.Label == "Zero").Ratio != 0) throw new Exception("Zero group lost");
        foreach (var axis in json.RootElement.GetProperty("axisCases").EnumerateArray())
            if (GraphNumeric.AxisCeiling(axis.GetProperty("rawMaximum").GetInt64()) != axis.GetProperty("ceiling").GetInt64()) throw new Exception("Axis fixture mismatch");
        Expect<InvalidDataException>(() => GraphNumeric.AxisCeiling(long.MaxValue));
        var a = json.RootElement.GetProperty("actionBars");
        var amounts = a.GetProperty("comparerOrderedAmounts").EnumerateArray().Select((x, i) => new GraphNumeric.BarAction(i.ToString("D2"), x.GetInt64(), DateTime.UnixEpoch)).ToArray();
        var bars = GraphNumeric.ActionBars(amounts.Reverse(), (x, y) => string.CompareOrdinal(x.Label, y.Label), (_, _) => 0);
        if (bars.RawMaximum != a.GetProperty("maximum").GetInt64() || bars.AxisCeiling != a.GetProperty("axisCeiling").GetInt64() || !bars.RelativeHeights.SequenceEqual(a.GetProperty("relativePixelHeights").EnumerateArray().Select(x => x.GetDouble()))) throw new Exception("Bars fixture mismatch");
        var ties = GraphNumeric.ActionBars([new("late", 1, DateTime.UnixEpoch.AddSeconds(1)), new("early", 1, DateTime.UnixEpoch)], (_, _) => 0, (_, _) => 0);
        var selectedCalls = 0;
        var sortedTies = GraphNumeric.ActionBars([new("b", 1, DateTime.UnixEpoch), new("a", 1, DateTime.UnixEpoch.AddSeconds(1))], (_, _) => { selectedCalls++; return 0; }, (x, y) => string.CompareOrdinal(x.Label, y.Label));
        if (selectedCalls < 2 || sortedTies.Ordered[0].Label != "a") throw new Exception("Comparer tie-break mismatch");
        if (ties.Ordered[0].Label != "early") throw new Exception("Time tie-break mismatch");
        if (GraphNumeric.ActionBars([], (_, _) => 0, (_, _) => 0).AxisCeiling != 2) throw new Exception("Empty bars mismatch");
        Expect<InvalidDataException>(() => GraphNumeric.ActionBars(Enumerable.Repeat(new GraphNumeric.BarAction("x", 1, DateTime.UnixEpoch), 16385), (_, _) => 0, (_, _) => 0));
        GraphSvgChecks.Run();
        Console.WriteLine("Production graph numeric arithmetic matches explicit ACT fixtures.");
    }
}
