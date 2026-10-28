using System.Xml;
using System.Xml.Linq;
using Aftercast.ActCompat;

internal static class GraphSvgChecks
{
    private static void Check(bool value, string message) { if (!value) throw new Exception(message); }
    private static void Invalid(Action action) { try { action(); } catch (Exception error) when (error is InvalidDataException or XmlException) { return; } throw new Exception("Expected invalid graph"); }
    internal static void Run()
    {
        var line = GraphSvg.Line("<line>&", [0, 5, 10]);
        var doc = XDocument.Parse(line); XNamespace svg = "http://www.w3.org/2000/svg";
        Check(doc.Root!.Element(svg + "title")!.Value == "<line>&", "title escaping");
        Check(doc.Descendants(svg + "polyline").Single().Attribute("points")!.Value == "40,220 330,120 620,20 ", "line coordinates");
        Invalid(() => GraphSvg.Line("bad", [-1]));
        Invalid(() => GraphSvg.Line("bad\0", []));
        Invalid(() => GraphSvg.Line("bad", Enumerable.Repeat(1L, 16385).ToArray()));
        var multiple = XDocument.Parse(GraphSvg.Lines("series", [new("A", new long[]{0,5}), new("B", new long[]{0,10})], 2));
        var multiPaths = multiple.Descendants(svg + "polyline").ToArray();
        Check(multiPaths[0].Attribute("points")!.Value == "40,220 620,120 " && multiPaths[1].Attribute("points")!.Value == "40,220 620,20 ", "common line axis");
        Check(multiple.Descendants(svg + "text").Any(x => x.Value == "2 s"), "line time axis");
        var shares = GraphNumeric.Distribution(100, [new("<Fire>", 60), new("Ice&", 30)]);
        var pie = XDocument.Parse(GraphSvg.Distribution("Pie", shares));
        Check(pie.Descendants(svg + "path").Count() == 3, "pie segmentation");
        Check(pie.Descendants(svg + "text").First().Value == "<Fire>: 0.6", "escaped pie labels");
        Check(XDocument.Parse(GraphSvg.Distribution("many", Enumerable.Repeat(new GraphNumeric.Share("zero", 0), 16).ToArray())).Root!.Attribute("viewBox")!.Value == "0 0 640 294", "legend range");
        Invalid(() => GraphSvg.Distribution("bad", [new("x", double.NaN)]));
        Invalid(() => GraphSvg.Distribution("bad", [new("x", -1)]));
        var bars = GraphNumeric.ActionBars([new("<hit>", 10, DateTime.UnixEpoch), new("miss", -10, DateTime.UnixEpoch)], (a,b) => string.CompareOrdinal(a.Label,b.Label), (_,_) => 0);
        var graph = XDocument.Parse(GraphSvg.Bars("Bars", bars));
        var rects = graph.Descendants(svg + "rect").ToArray();
        Check(rects[0].Attribute("y")!.Value == "50" && rects[0].Attribute("height")!.Value == "150", "positive bar");
        Check(rects[1].Attribute("y")!.Value == "200" && rects[1].Attribute("height")!.Value == "150", "negative bar");
        Check(graph.Root!.Attribute("viewBox")!.Value == "0 0 640 355", "negative extent");
        Check(rects[0].Element(svg + "title")!.Value == "<hit>: 10", "bar title escaping");
        Check(XDocument.Parse(GraphSvg.Bars("empty", GraphNumeric.ActionBars([], (_,_) => 0, (_,_) => 0))).Root is not null, "empty graph");
        Console.WriteLine("Static SVG graph coordinates, bounds and escaping passed.");
    }
}
