namespace Aftercast.ActCompat;

/// <summary>ACT line-series arithmetic. Selection and rendering remain caller responsibilities.</summary>
public static class GraphNumeric
{
    public sealed record Sample(double Seconds, long Damage);
    public sealed record Series(long[] Raw, long[] Rates, long[] Plotted);

    public static Series Line(int duration, int window, bool rolling, IEnumerable<Sample> samples)
    {
        ArgumentNullException.ThrowIfNull(samples);
        if (duration < 0 || duration > 360000)
            throw new ArgumentOutOfRangeException(nameof(duration));
        if (window is < 1 or > 120)
            throw new ArgumentOutOfRangeException(nameof(window));
        var raw = new long[rolling ? checked(duration + 1) : checked(duration / window + 2)];
        var count = 0;
        foreach (var sample in samples)
        {
            if (++count > 16384) throw new InvalidDataException("Graph exceeds 16384 actions");
            if (sample is null) throw new InvalidDataException("Invalid graph sample");
            if (sample.Damage <= 0) continue;
            if (!double.IsFinite(sample.Seconds) || sample.Seconds < int.MinValue || sample.Seconds > int.MaxValue)
                throw new InvalidDataException("Invalid graph timestamp");
            var offset = (int)sample.Seconds;
            if (rolling)
            {
                if (offset < 0) continue;
                for (var i = offset; i < (long)offset + window && i < duration; i++)
                    raw[i] = unchecked(raw[i] + sample.Damage);
            }
            else
            {
                var index = offset / window + 1;
                if (index < 0 || index >= raw.Length) throw new InvalidDataException("Graph bucket outside allocated range");
                raw[index] = unchecked(raw[index] + sample.Damage);
            }
        }
        var rates = raw.Select(value => value / window).ToArray();
        var countPlotted = rolling ? duration : duration / window;
        return new(raw, rates, rates.Take(countPlotted).ToArray());
    }
    public sealed record Share(string Label, double Ratio);
    public sealed record CategoryValue(string Label, double Value);
    public sealed record ResistAction(string DamageType, long Damage);
    public sealed record BarAction(string Label, long Damage, DateTime Time);
    public sealed record Bars(BarAction[] Ordered, long RawMaximum, long AxisCeiling, double[] RelativeHeights);

    // Invalid geometry is rejected rather than silently normalizing the denominator.
    public static Share[] Distribution(double denominator, IEnumerable<CategoryValue> values)
    {
        ArgumentNullException.ThrowIfNull(values);
        if (!double.IsFinite(denominator) || denominator <= 0) throw new InvalidDataException("Invalid distribution denominator");
        var result = new List<Share>();
        var count = 0;
        foreach (var value in values)
        {
            if (++count > 16384) throw new InvalidDataException("Graph exceeds 16384 categories");
            if (value is null || value.Label is null || !double.IsFinite(value.Value)) throw new InvalidDataException("Invalid category value");
            if (value.Value > 0)
            {
                var ratio = value.Value / denominator;
                if (!double.IsFinite(ratio)) throw new InvalidDataException("Invalid distribution ratio");
                result.Add(new(value.Label, ratio));
            }
        }
        return result.ToArray();
    }

    public static Share[] Resist(IEnumerable<ResistAction> actions, string wardPrefix, string allLabel)
    {
        ArgumentNullException.ThrowIfNull(actions);
        ArgumentNullException.ThrowIfNull(wardPrefix);
        ArgumentNullException.ThrowIfNull(allLabel);
        var groups = new Dictionary<string, long>(StringComparer.Ordinal);
        long total = 0;
        var count = 0;
        foreach (var action in actions)
        {
            if (++count > 16384) throw new InvalidDataException("Graph exceeds 16384 actions");
            if (action is null || action.DamageType is null) throw new InvalidDataException("Invalid resist action");
            var name = action.DamageType;
            if (name.StartsWith(wardPrefix, StringComparison.CurrentCulture))
            {
                if (name.Length < 7) throw new InvalidDataException("Ward label shorter than seven UTF-16 units");
                name = name.Substring(7);
            }
            groups.TryAdd(name, 0);
            if (action.Damage > 0)
            {
                try
                {
                    groups[name] = checked(groups[name] + action.Damage);
                    total = checked(total + action.Damage);
                    if (name == allLabel) total = checked(total + action.Damage);
                }
                catch (OverflowException error) { throw new InvalidDataException("Resist total overflow", error); }
            }
        }
        if (total <= 0) throw new InvalidDataException("Invalid resist total");
        return groups.Where(pair => pair.Key != allLabel).Select(pair => new Share(pair.Key, (double)pair.Value / total)).ToArray();
    }

    public static Bars ActionBars(IEnumerable<BarAction> actions, Comparison<BarAction> selected, Comparison<BarAction> timeSorter)
    {
        ArgumentNullException.ThrowIfNull(actions);
        ArgumentNullException.ThrowIfNull(selected);
        ArgumentNullException.ThrowIfNull(timeSorter);
        var list = new List<BarAction>();
        foreach (var action in actions)
        {
            if (list.Count == 16384) throw new InvalidDataException("Graph exceeds 16384 actions");
            if (action is null || action.Label is null) throw new InvalidDataException("Invalid bar action");
            list.Add(action);
        }
        list.Sort((a, b) => {
            var result = selected(a, b);
            if (result == 0) result = selected(a, b);
            if (result == 0) result = timeSorter(a, b);
            return result == 0 ? a.Time.CompareTo(b.Time) : result;
        });
        var maximum = Math.Max(1, list.Count == 0 ? 1 : list.Max(action => action.Damage));
        var ceiling = AxisCeiling(maximum);
        return new(list.ToArray(), maximum, ceiling, list.Select(action => (double)action.Damage / ceiling).ToArray());
    }

    public static long AxisCeiling(long rawMaximum)
    {
        var maximum = Math.Max(1, rawMaximum);
        long ceiling = 10;
        while (ceiling <= maximum)
        {
            if (ceiling > long.MaxValue / 10) throw new InvalidDataException("Graph axis exceeds supported range");
            ceiling *= 10;
        }
        while (ceiling / 2 > maximum) ceiling /= 2;
        if (ceiling / 1.25 > maximum) ceiling = Convert.ToInt64(ceiling / 1.25);
        return ceiling;
    }
}
