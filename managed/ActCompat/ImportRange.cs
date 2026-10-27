namespace Aftercast.ActCompat;

/// <summary>ACT history reparse bounds: inclusive source wall-clock ticks, without timezone conversion.</summary>
public sealed record ImportRange
{
    public DateTime Start { get; }
    public DateTime End { get; }
    public bool ExtendToCombatEnd { get; }
    public ImportRange(DateTime start, DateTime end, bool extendToCombatEnd = false)
    {
        if (start > end) throw new ArgumentException("Import start must not follow end");
        Start = start;
        End = end; ExtendToCombatEnd = extendToCombatEnd;
    }
}
