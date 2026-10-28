using Advanced_Combat_Tracker;

namespace Aftercast.ActCompat;

/// <summary>ACT timeline's positive-value bins, before grouping or drawing.</summary>
public static class TimelineNumeric
{
    public sealed record Sample(double Seconds, long Amount);
    public sealed record Series(long[] Raw, long[] DisplayedRates);

    public static Series Calculate(int duration, int window, bool rolling, IEnumerable<Sample> samples)
    {
        ArgumentNullException.ThrowIfNull(samples);
        if (duration < 0 || duration > 360000) throw new ArgumentOutOfRangeException(nameof(duration));
        if (window is < 1 or > 600) throw new ArgumentOutOfRangeException(nameof(window));
        var points = rolling ? duration : duration / window;
        var raw = new long[checked(points * 2)];
        var count = 0;
        foreach (var sample in samples)
        {
            if (++count > 16384) throw new InvalidDataException("Timeline exceeds 16384 samples");
            if (sample is null) throw new InvalidDataException("Invalid timeline sample");
            if (sample.Amount <= 0) continue;
            if (!double.IsFinite(sample.Seconds) || sample.Seconds < int.MinValue || sample.Seconds > int.MaxValue)
                throw new InvalidDataException("Invalid timeline time");
            var offset = (int)sample.Seconds;
            if (rolling)
            {
                for (var i = offset; offset >= 0 && i < (long)offset + window && i < duration; i++)
                    raw[i] = (long)((Dnum)raw[i] + (Dnum)sample.Amount);
            }
            else
            {
                var index = offset / window + 1;
                if (index < 0 || index >= raw.Length) throw new InvalidDataException("Timeline sample outside ACT bucket bounds");
                raw[index] = (long)((Dnum)raw[index] + (Dnum)sample.Amount);
            }
        }
        var displayed = new long[points];
        for (var i = 1; i <= points; i++)
        {
            if (i >= raw.Length) throw new InvalidDataException("Timeline display outside ACT bucket bounds");
            displayed[i - 1] = raw[i] / window;
        }
        return new(raw, displayed);
    }
}
