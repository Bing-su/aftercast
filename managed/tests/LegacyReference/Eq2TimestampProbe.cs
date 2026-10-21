using System.Reflection;
using System.Runtime.CompilerServices;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class Eq2TimestampProbe
{
    public static string Run()
    {
        var main = RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        GC.SuppressFinalize(main);
        var parser = typeof(FormActMain).GetMethod("ParseDateTimeFromEQ2Log", BindingFlags.Instance | BindingFlags.Static | BindingFlags.NonPublic)!;
        DateTime Parse(string? text) => (DateTime)parser.Invoke(parser.IsStatic ? null : main, new object?[] { text })!;
        var date = Parse("(0000000000)[Fri Dec 02 03:04:05 2026] fixture");
        if (date != new DateTime(2026, 12, 2, 3, 4, 5) || date.Kind != DateTimeKind.Unspecified ||
            Parse("(9999999999)[Fri XXX 02 03:04:05 2026] fixture") != new DateTime(2026, 1, 2, 3, 4, 5) ||
            Parse("(0000000000)[Fri Jan 99 03:04:05 2026] fixture") != DateTime.MinValue ||
            Parse("short") != DateTime.MinValue || Parse(null) != DateTime.MinValue)
            throw new Exception("Original-reference EQ2 timestamp mismatch");
        return "EQ2 timestamps";
    }
}
