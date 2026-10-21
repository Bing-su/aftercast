namespace Advanced_Combat_Tracker;

public class TextExportFormatOptions(string playerFormat, string sorting, bool showOnlyAllies,
    bool showAlliedInfo, string alliesFormat) : IEquatable<TextExportFormatOptions>
{
    public string PlayerFormat { get; } = playerFormat;
    public string Sorting { get; } = sorting;
    public bool ShowOnlyAllies { get; } = showOnlyAllies;
    public bool ShowAlliedInfo { get; } = showAlliedInfo;
    public string AlliesFormat { get; } = alliesFormat;
    public override string ToString() => $"({Sorting}) \"{PlayerFormat}\"";
    public override int GetHashCode() => $"{PlayerFormat}{AlliesFormat}{Sorting}{ShowOnlyAllies}{ShowAlliedInfo}".GetHashCode();
    // ACT compares format hashes, including collisions; plugins use this equality for duplicate registration.
    public bool Equals(TextExportFormatOptions? other) => GetHashCode().Equals(other!.GetHashCode());
    public override bool Equals(object? other) => other != DBNull.Value && Equals((TextExportFormatOptions?)other);
}
