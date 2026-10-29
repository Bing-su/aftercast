using System.Drawing;
using System.Reflection;

// Executes only ACT's pure four-argument color transform against synthetic inputs.
internal static class ColorAdjustProbe
{
    internal static object Run(Assembly act, string fixture)
    {
        var type = act.GetType("Advanced_Combat_Tracker.Extensions", throwOnError: true)!;
        var method = type.GetMethods(BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic)
            .Single(x => x.Name == "ToAdjustedColor" && x.GetParameters().Length == 4);
        var rows = 0;
        foreach (var line in File.ReadLines(fixture))
        {
            var parts = line.Split(' ').Select(int.Parse).ToArray();
            if (parts.Length != 5) throw new InvalidDataException("Invalid synthetic color fixture");
            var actual = (Color)method.Invoke(null, [Color.FromArgb(parts[0]), parts[1] == 1, parts[2] / 100f, parts[3] / 100f])!;
            if (actual.ToArgb() != parts[4]) throw new InvalidDataException($"Original color mismatch at row {rows}");
            rows++;
        }
        if (rows != 1024) throw new InvalidDataException("Unexpected synthetic color fixture count");
        return new { rows };
    }
}
