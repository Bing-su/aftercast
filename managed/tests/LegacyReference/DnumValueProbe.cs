using System.Globalization;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

// Exercise original member references; expected terms are ACT's built-in English defaults.
public static class DnumValueProbe
{
    private sealed class Derived(long number, string text) : Dnum(number, text) { }

    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Original-reference Dnum value probe failed");
    }

    private static void Throws<T>(Action action) where T : Exception
    {
        try { action(); }
        catch (T) { return; }
        throw new Exception($"Dnum operation did not throw {typeof(T).Name}");
    }

    public static string Run()
    {
        var culture = CultureInfo.CurrentCulture;
        var commas = ActGlobals.mainTableShowCommas;
        try
        {
            foreach (var name in new[] { "en-US", "fr-FR", "ko-KR" })
            {
                CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo(name);
                foreach (var grouped in new[] { false, true })
                {
                    ActGlobals.mainTableShowCommas = grouped;
                    foreach (var number in new[] { 1L, 1234567L, long.MaxValue })
                    {
                        var value = new Dnum(number);
                        var text = number.ToString(grouped ? "#,0" : "0");
                        Check(value.ToString() == text && value.DamageString == text);
                        Check(value.ToString(false) == text && value.ToString(true) == number.ToString());
                        Check(value.GetHashCode() == text.GetHashCode());
                    }
                    var labelled = new Dnum(10, "First") { DamageString2 = " second" };
                    Check(labelled.ToString() == "First second" && labelled.DamageString == "First");
                    Check(labelled.ToString(false) == "10" && labelled.ToString(true) == "10");
                    Check(labelled.GetHashCode() == "10".GetHashCode());
                }
            }

            foreach (var (number, term) in new (long, string)[]
            {
                (0, "No Damage"), (-1, "Miss"), (-2, "Resist"), (-3, "Parry"),
                (-4, "Riposte"), (-5, "Block"), (-10, "Death")
            })
            {
                var value = new Dnum(number, "Explicit") { DamageString2 = " suffix" };
                Check(value.ToString() == term && value.DamageString == "Explicit");
                var detailed = number is 0 or -1 or -10 ? term : "Explicit";
                var compact = number == 0 ? "0" : number is -1 or -10 ? term : "Exp";
                Check(value.ToString(false) == detailed && value.ToString(true) == compact);
                Check(value.GetHashCode() == detailed.GetHashCode());
            }

            var first = new Dnum(10, "First");
            var positiveNull = new Dnum(42, null!);
            Check(positiveNull.ToString() == "42" && positiveNull.DamageString == "42" && positiveNull.ToString(false) == "42");
            Check(new Dnum(-9, "AB").ToString(true) == "AB");
            Check(new Dnum(-9, "A").CompareTo(new Dnum(-9, null!)) == "A".CompareTo(null));
            var same = new Dnum(10, "First") { DamageString2 = "ignored by equality" };
            var different = new Dnum(10, "Second");
            Check(first.Equals(same) && first == same && !(first != same));
            Check(!first.Equals(different) && first != different && !(first == different));
            Check(first.Equals(new Derived(10, "First")));
            Check(((IComparable)first).CompareTo(different) == 0);
            Check(new Dnum(-9, "A").CompareTo(new Dnum(-9, "B")) == "A".CompareTo("B"));
            Throws<NullReferenceException>(() => first.Equals(null));
            Throws<InvalidCastException>(() => first.Equals(10L));
            Throws<NullReferenceException>(() => first.CompareTo(null));
            Throws<InvalidCastException>(() => first.CompareTo("wrong type"));
            Dnum leftNull = null!, rightNull = null!;
            Throws<NullReferenceException>(() => _ = leftNull == rightNull);
            Throws<NullReferenceException>(() => _ = leftNull != first);
            Throws<NullReferenceException>(() => _ = first == rightNull);
            Throws<NullReferenceException>(() => new Dnum(-9, null!).ToString(true));
            Throws<NullReferenceException>(() => new Dnum(-9, null!).GetHashCode());
            Throws<NullReferenceException>(() => new Dnum(-9, null!).CompareTo(new Dnum(-9, "B")));

            Check((first + new Dnum(5)).Number == 15 && (first + new Dnum(5)).DamageString == new Dnum(15).ToString());
            Check((new Dnum(-1) + first).Number == 10 && (first + new Dnum(-9)).Number == 10);
            Check((new Dnum(-9) + new Dnum(-10)).Number == 0);
            Check((new Dnum(long.MaxValue) + new Dnum(1)).Number == long.MinValue);
            Check(new Dnum(long.MinValue).ToString() == "" && new Dnum(-11).ToString() == "");
            return "Dnum values";
        }
        finally
        {
            CultureInfo.CurrentCulture = culture;
            ActGlobals.mainTableShowCommas = commas;
        }
    }
}
