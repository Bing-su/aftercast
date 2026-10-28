using System.Globalization;
using System.Text.Json;

// An executable transcription of inspected IL, not a production graph or native drawing test.
internal static class SimpleGraphContractChecks
{
    internal static void Run()
    {
        using var input=typeof(SimpleGraphContractChecks).Assembly.GetManifestResourceStream("SimpleGraphContracts.json") ?? throw new Exception("Missing Simple fixtures");
        using var json=JsonDocument.Parse(input);
        foreach(var test in json.RootElement.GetProperty("cases").EnumerateArray())
        {
            var rows=test.GetProperty("rows").EnumerateArray().Reverse().ToArray();
            var only=test.GetProperty("onlyAllies").GetBoolean();
            var allAllies=test.GetProperty("selective").GetBoolean()&&test.GetProperty("ignoreEnemies").GetBoolean();
            var hasAllies=allAllies?rows.Length>0:rows.Any(row=>row[2].GetBoolean());
            var names=new List<string>();var values=new List<double>();double total=0;
            foreach(var row in rows)
            {
                if(only&&hasAllies&&!allAllies&&!row[2].GetBoolean())continue;
                if(test.GetProperty("exportSelected").GetBoolean()&&!row[3].GetBoolean())continue;
                try
                {
                    if(row[1].GetString()=="callback-error")throw new IOException("synthetic callback");
                    var value=double.Parse(row[1].GetString()!,CultureInfo.GetCultureInfo("en-US"));
                    if(!(value>0))continue;
                    names.Add(row[0].GetString()!);values.Add(value);total+=value;
                }
                catch(Exception){ }
            }
            if(!names.SequenceEqual(test.GetProperty("expected").EnumerateArray().Select(value=>value.GetString())))throw new Exception("Simple ordering/filter: "+test.GetProperty("name"));
            if(values.Count==0)
            {
                var message=rows.Length==0?"no-data":"no-damage";
                if(message!=test.GetProperty("noData").GetString())throw new Exception("Simple no-data branch");
                continue;
            }
            var production=Aftercast.ActCompat.GraphNumeric.Simple(names.Zip(values,(name,value)=>new Aftercast.ActCompat.GraphNumeric.SimpleValue(name,value))) ?? throw new Exception("Missing production Simple result");
            if(!production.Values.Select(value=>value.Name).SequenceEqual(names))throw new Exception("Production Simple ordering");
            double maximum=1;foreach(var value in values)if(value>maximum)maximum=Math.Ceiling(value);
            if(production.Axis!=maximum*1.1||production.Total!=total||production.Average!=total/values.Count)throw new Exception("Production Simple numeric mismatch");
            static bool Equal(double a,double b)=>Math.Abs(a-b)<=1e-10*Math.Max(1,Math.Abs(b));
            if(!Equal(maximum*1.1,test.GetProperty("axis").GetDouble())||!Equal(total,test.GetProperty("total").GetDouble())||!Equal(total/values.Count,test.GetProperty("average").GetDouble()))throw new Exception("Simple numeric fixture: "+test.GetProperty("name"));
        }
        foreach(var test in json.RootElement.GetProperty("totals").EnumerateArray())
        {
            var actual=string.Format(CultureInfo.GetCultureInfo(test.GetProperty("culture").GetString()!),"Total: {0:0,0}\nAVG: {1:0,0.00}",test.GetProperty("sum").GetDouble(),test.GetProperty("sum").GetDouble()/test.GetProperty("count").GetInt32());
            if(actual!=test.GetProperty("text").GetString())throw new Exception("Simple totals formatting: "+actual);
        }
        if(Aftercast.ActCompat.GraphNumeric.Simple([new("zero",0),new("negative",-1)]) is not null)throw new Exception("Simple no-data");
        try { Aftercast.ActCompat.GraphNumeric.Simple([new("bad",double.NaN)]);throw new Exception("Expected nonfinite rejection"); } catch(InvalidDataException) { }
        try { Aftercast.ActCompat.GraphNumeric.Simple([new("bad",double.PositiveInfinity)]);throw new Exception("Expected infinity rejection"); } catch(InvalidDataException) { }
        try { Aftercast.ActCompat.GraphNumeric.Simple([new("a",double.MaxValue)]);throw new Exception("Expected axis overflow rejection"); } catch(InvalidDataException) { }
        try { Aftercast.ActCompat.GraphNumeric.Simple([new("a",double.MaxValue),new("b",double.MaxValue)]);throw new Exception("Expected overflow rejection"); } catch(InvalidDataException) { }
        _=Aftercast.ActCompat.GraphNumeric.Simple(Enumerable.Repeat(new Aftercast.ActCompat.GraphNumeric.SimpleValue("a",1),16384));
        try { Aftercast.ActCompat.GraphNumeric.Simple(Enumerable.Repeat(new Aftercast.ActCompat.GraphNumeric.SimpleValue("a",1),16385));throw new Exception("Expected row limit"); } catch(InvalidDataException) { }
        var previous=CultureInfo.CurrentCulture;
        try
        {
            CultureInfo.CurrentCulture=CultureInfo.GetCultureInfo("de-DE");
            var svg=Aftercast.ActCompat.GraphSvg.Simple("<Simple>",Aftercast.ActCompat.GraphNumeric.Simple([new("<A>",1234.5)])!,true);
            if(!svg.Contains("&lt;Simple&gt;",StringComparison.Ordinal)||!svg.Contains("Total: 1.235",StringComparison.Ordinal)||!svg.Contains("AVG: 1.234,50",StringComparison.Ordinal)||!svg.Contains("<tspan",StringComparison.Ordinal))throw new Exception("Simple SVG escaping or culture");
        }
        finally { CultureInfo.CurrentCulture=previous; }
        var infinity=double.Parse(CultureInfo.GetCultureInfo("en-US").NumberFormat.PositiveInfinitySymbol,CultureInfo.GetCultureInfo("en-US"));
        if(!(infinity>0)||!double.IsPositiveInfinity(Math.Ceiling(infinity)*1.1)||double.Parse("NaN",CultureInfo.GetCultureInfo("en-US"))>0)throw new Exception("Simple unordered/Infinity semantics");
        if(!double.IsPositiveInfinity(double.MaxValue+double.MaxValue))throw new Exception("Simple sum overflow semantics");
        Console.WriteLine("Simple graph IL transcription fixtures passed; production integration and native drawing untested.");
    }
}
