using System.Globalization;
using System.Net;
using System.Text;
using System.Xml;

namespace Aftercast.ActCompat;

/// <summary>Bounded static SVG fragments, without scripts, styles or external resources.</summary>
public static class GraphSvg
{
    private const int Limit = 16384;
    private const int MaxBytes = 2 * 1024 * 1024;
    private static readonly string[] Colors = ["#3568a8", "#bd562d", "#328452", "#854e9c", "#9a7721", "#347e83"];
    private static string N(double value)
    {
        if (!double.IsFinite(value)) throw new InvalidDataException("Nonfinite graph coordinate");
        return value.ToString("0.########", CultureInfo.InvariantCulture);
    }
    private static string E(string value)
    {
        if (value.Length > 4096) throw new InvalidDataException("Graph label exceeds 4096 characters");
        XmlConvert.VerifyXmlChars(value);
        return WebUtility.HtmlEncode(value);
    }
    private static StringBuilder Start(string label, double height = 256)
    {
        ArgumentNullException.ThrowIfNull(label);
        var b = new StringBuilder();
        b.Append("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 640 ").Append(N(height)).Append("\" width=\"640\" height=\"").Append(N(height)).Append("\" role=\"img\"><title>").Append(E(label)).Append("</title>");
        return b;
    }
    private static string Finish(StringBuilder b)
    {
        b.Append("</svg>");
        if (Encoding.UTF8.GetByteCount(b.ToString()) > MaxBytes) throw new InvalidDataException("Graph SVG exceeds 2 MiB");
        return b.ToString();
    }
    public static string Line(string label, IReadOnlyList<long> values)
    {
        ArgumentNullException.ThrowIfNull(values);
        if (values.Count > Limit) throw new InvalidDataException("Graph exceeds 16384 points");
        if (values.Any(value => value < 0)) throw new InvalidDataException("Negative line value cannot be rendered");
        var b = Start(label); long max = Math.Max(1, values.Count == 0 ? 1 : values.Max());
        b.Append("<text x=\"8\" y=\"18\">max ").Append(max.ToString(CultureInfo.InvariantCulture)).Append("</text><path d=\"M40 20V220H620\" fill=\"none\" stroke=\"black\"/>");
        if (values.Count != 0)
        {
            b.Append("<polyline fill=\"none\" stroke=\"#3568a8\" points=\"");
            for (var i = 0; i < values.Count; i++)
            {
                var x = 40.0 + (values.Count == 1 ? 0 : 580.0 * i / (values.Count - 1));
                var y = 220.0 - 200.0 * values[i] / max;
                b.Append(N(x)).Append(',').Append(N(y)).Append(' ');
            }
            b.Append("\"/>");
        }
        b.Append("<text x=\"40\" y=\"242\">0</text><text x=\"580\" y=\"242\">").Append(Math.Max(0, values.Count - 1).ToString(CultureInfo.InvariantCulture)).Append("</text>");
        return Finish(b);
    }
    public sealed record LineSeries(string Label, IReadOnlyList<long> Values);
    public static string Lines(string label, IReadOnlyList<LineSeries> series, int secondsPerPoint)
    {
        ArgumentNullException.ThrowIfNull(series);
        if (secondsPerPoint < 1 || series.Count > 256) throw new InvalidDataException("Invalid line graph series");
        var points = 0; long maximum = 1; var longest = 0;
        foreach (var item in series)
        {
            if (item is null || item.Label is null || item.Values is null) throw new InvalidDataException("Invalid line graph series");
            points = checked(points + item.Values.Count);
            if (points > Limit) throw new InvalidDataException("Graph exceeds 16384 points");
            foreach (var value in item.Values) { if (value < 0) throw new InvalidDataException("Negative line value cannot be rendered"); maximum = Math.Max(maximum, value); }
            longest = Math.Max(longest, item.Values.Count);
        }
        var b = Start(label, Math.Max(256, 260 + series.Count * 16.0));
        b.Append("<text x=\"8\" y=\"18\">max ").Append(maximum.ToString(CultureInfo.InvariantCulture)).Append("</text><path d=\"M40 20V220H620\" fill=\"none\" stroke=\"black\"/>");
        for (var row = 0; row < series.Count; row++)
        {
            var item = series[row];
            b.Append("<polyline fill=\"none\" stroke=\"").Append(Colors[row % Colors.Length]).Append("\" points=\"");
            for (var i = 0; i < item.Values.Count; i++)
                b.Append(N(40.0 + (longest <= 1 ? 0 : 580.0 * i / (longest - 1)))).Append(',').Append(N(220.0 - 200.0 * item.Values[i] / maximum)).Append(' ');
            b.Append("\"/><text x=\"40\" y=\"").Append(N(260 + row * 16.0)).Append("\" fill=\"").Append(Colors[row % Colors.Length]).Append("\">").Append(E(item.Label)).Append("</text>");
            if (b.Length > MaxBytes) throw new InvalidDataException("Graph SVG exceeds 2 MiB");
        }
        b.Append("<text x=\"40\" y=\"242\">0</text><text x=\"550\" y=\"242\">").Append(((long)Math.Max(0, longest - 1) * secondsPerPoint).ToString(CultureInfo.InvariantCulture)).Append(" s</text>");
        return Finish(b);
    }
    public static string Distribution(string label, IReadOnlyList<GraphNumeric.Share> shares)
    {
        ArgumentNullException.ThrowIfNull(shares);
        if (shares.Count > Limit) throw new InvalidDataException("Graph exceeds 16384 shares");
        var b = Start(label, Math.Max(256, 38 + shares.Count * 16.0)); var angle = -Math.PI / 2;
        for (var i = 0; i < shares.Count; i++)
        {
            var share = shares[i] ?? throw new InvalidDataException("Invalid graph share");
            if (share.Label is null || !double.IsFinite(share.Ratio) || share.Ratio < 0 || share.Ratio > 16) throw new InvalidDataException("Invalid graph share ratio");
            var remaining = share.Ratio * 2 * Math.PI;
            while (remaining > 0.000000001)
            {
                var sweep = Math.Min(remaining, Math.PI); var end = angle + sweep;
                b.Append("<path fill=\"").Append(Colors[i % Colors.Length]).Append("\" d=\"M160 128 L").Append(N(160 + 100 * Math.Cos(angle))).Append(' ').Append(N(128 + 100 * Math.Sin(angle))).Append(" A100 100 0 0 1 ").Append(N(160 + 100 * Math.Cos(end))).Append(' ').Append(N(128 + 100 * Math.Sin(end))).Append(" Z\"><title>").Append(E(share.Label)).Append(": ").Append(N(share.Ratio)).Append("</title></path>");
                angle = end; remaining -= sweep;
                if (b.Length > MaxBytes) throw new InvalidDataException("Graph SVG exceeds 2 MiB");
            }
            // Labels remain visible even for zero-sized slices.
            b.Append("<text x=\"280\" y=\"").Append(N(22 + i * 16.0)).Append("\" fill=\"").Append(Colors[i % Colors.Length]).Append("\">").Append(E(share.Label)).Append(": ").Append(N(share.Ratio)).Append("</text>");
            if (b.Length > MaxBytes) throw new InvalidDataException("Graph SVG exceeds 2 MiB");
        }
        return Finish(b);
    }
    public static string Empty(string label, string message)
    {
        var b = Start(label); b.Append("<text x=\"40\" y=\"128\">").Append(E(message)).Append("</text>"); return Finish(b);
    }
    public static string Simple(string label, GraphNumeric.SimpleResult data, bool totals)
    {
        ArgumentNullException.ThrowIfNull(data);
        if (data.Values.Length > Limit || data.Axis <= 0 || !double.IsFinite(data.Axis)) throw new InvalidDataException("Invalid Simple graph");
        var b = Start(label, Math.Max(256, 260 + data.Values.Length * 16.0));
        b.Append("<path d=\"M40 20V200H620\" fill=\"none\" stroke=\"black\"/><text x=\"8\" y=\"18\">").Append(N(data.Axis)).Append("</text>");
        var width = 580.0 / Math.Max(1, data.Values.Length);
        for (var i = 0; i < data.Values.Length; i++)
        {
            var value = data.Values[i]; var height = (value.Value / data.Axis) * 180;
            b.Append("<rect fill=\"#3568a8\" x=\"").Append(N(40 + width * i)).Append("\" y=\"").Append(N(200-height)).Append("\" width=\"").Append(N(width)).Append("\" height=\"").Append(N(height)).Append("\"><title>").Append(E(value.Name)).Append(": ").Append(N(value.Value)).Append("</title></rect><text x=\"40\" y=\"").Append(N(260+i*16)).Append("\">").Append(E(value.Name)).Append("</text>");
            if (b.Length > MaxBytes) throw new InvalidDataException("Graph SVG exceeds 2 MiB");
        }
        if (totals) b.Append("<text x=\"400\" y=\"40\"><tspan x=\"400\">Total: ").Append(E(data.Total.ToString("0,0",CultureInfo.CurrentCulture))).Append("</tspan><tspan x=\"400\" dy=\"16\">AVG: ").Append(E(data.Average.ToString("0,0.00",CultureInfo.CurrentCulture))).Append("</tspan></text>");
        return Finish(b);
    }
    public static string Bars(string label, GraphNumeric.Bars bars)
    {
        ArgumentNullException.ThrowIfNull(bars);
        if (bars.Ordered.Length != bars.RelativeHeights.Length || bars.Ordered.Length > Limit || bars.AxisCeiling <= 0) throw new InvalidDataException("Invalid bar graph");
        var below = bars.RelativeHeights.Length == 0 ? 0 : Math.Max(0, -bars.RelativeHeights.Min() * 180);
        if (!double.IsFinite(below) || below > 1000000) throw new InvalidDataException("Graph negative range exceeds limit");
        var b = Start(label, Math.Max(256, 205 + below));
        b.Append("<text x=\"8\" y=\"18\">max ").Append(bars.AxisCeiling.ToString(CultureInfo.InvariantCulture)).Append("</text><path d=\"M40 20V").Append(N(200 + below)).Append(" M40 200H620\" fill=\"none\" stroke=\"black\"/>");
        if (below > 0) b.Append("<text x=\"2\" y=\"").Append(N(200 + below)).Append("\">min ").Append(bars.Ordered.Min(item => item.Damage).ToString(CultureInfo.InvariantCulture)).Append("</text>");
        var width = 580.0 / Math.Max(1, bars.Ordered.Length);
        for (var i = 0; i < bars.Ordered.Length; i++)
        {
            var value = bars.RelativeHeights[i]; if (!double.IsFinite(value)) throw new InvalidDataException("Invalid bar height");
            var height = 180 * value; var y = height >= 0 ? 200 - height : 200;
            b.Append("<rect fill=\"#3568a8\" x=\"").Append(N(40 + width * i)).Append("\" y=\"").Append(N(y)).Append("\" width=\"").Append(N(width)).Append("\" height=\"").Append(N(Math.Abs(height))).Append("\"><title>").Append(E(bars.Ordered[i].Label)).Append(": ").Append(bars.Ordered[i].Damage.ToString(CultureInfo.InvariantCulture)).Append("</title></rect>");
            if (b.Length > MaxBytes) throw new InvalidDataException("Graph SVG exceeds 2 MiB");
        }
        return Finish(b);
    }
}
