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
            double maximum=1;foreach(var value in values)if(value>maximum)maximum=Math.Ceiling(value);
            static bool Equal(double a,double b)=>Math.Abs(a-b)<=1e-10*Math.Max(1,Math.Abs(b));
            if(!Equal(maximum*1.1,test.GetProperty("axis").GetDouble())||!Equal(total,test.GetProperty("total").GetDouble())||!Equal(total/values.Count,test.GetProperty("average").GetDouble()))throw new Exception("Simple numeric fixture: "+test.GetProperty("name"));
        }
        foreach(var test in json.RootElement.GetProperty("totals").EnumerateArray())
        {
            var actual=string.Format(CultureInfo.GetCultureInfo(test.GetProperty("culture").GetString()!),"Total: {0:0,0}\nAVG: {1:0,0.00}",test.GetProperty("sum").GetDouble(),test.GetProperty("sum").GetDouble()/test.GetProperty("count").GetInt32());
            if(actual!=test.GetProperty("text").GetString())throw new Exception("Simple totals formatting: "+actual);
        }
        var infinity=double.Parse(CultureInfo.GetCultureInfo("en-US").NumberFormat.PositiveInfinitySymbol,CultureInfo.GetCultureInfo("en-US"));
        if(!(infinity>0)||!double.IsPositiveInfinity(Math.Ceiling(infinity)*1.1)||double.Parse("NaN",CultureInfo.GetCultureInfo("en-US"))>0)throw new Exception("Simple unordered/Infinity semantics");
        if(!double.IsPositiveInfinity(double.MaxValue+double.MaxValue))throw new Exception("Simple sum overflow semantics");
        Console.WriteLine("Simple graph IL transcription fixtures passed; production integration and native drawing untested.");
    }
}
