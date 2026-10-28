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
}
