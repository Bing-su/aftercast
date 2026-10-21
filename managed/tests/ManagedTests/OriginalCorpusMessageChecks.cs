internal static class OriginalCorpusMessageChecks
{
    internal static void Run()
    {
        var directory = Path.Combine(Path.GetTempPath(), "bxivact corpus messages");
        var plugin = Path.Combine(directory, "FFXIV_ACT_Plugin.dll");
        string Message(string body) => "[12:34:56.789] " + body;
        var baseline = new[]
        {
            Message("FFXIV_ACT_Plugin Version: 3.0.3.3"),
            Message("Selected Language ID: English"),
            Message("Selected Process ID: 0")
        };
        string Renamed(string source, string target) => Message($"Renamed [{source}] to [{target}]].");
        var source = Path.Combine(directory, "deucalion-1.5.0.distrib.dll");
        var target = Path.Combine(directory, "deucalion-1.5.0.dll");
        OriginalCorpusChecks.CheckMessages(baseline, plugin);
        // A pristine trusted deployment renames its bundled DLL once; subsequent starts omit this notice.
        OriginalCorpusChecks.CheckMessages([.. baseline, Renamed(source, target)], plugin);
        OriginalCorpusChecks.CheckMessages([.. baseline,
            Message("SimulatedDamage Timeout with no ticks/GroundAoE: Removing status fixture")], plugin);
        foreach (var unexpected in new[]
        {
            Renamed(Path.Combine(directory, "other.distrib.dll"), target),
            Renamed(source, Path.Combine(directory, "other.dll")),
            Renamed(Path.Combine(directory, "other", "deucalion-1.5.0.distrib.dll"), target),
            Message("System.InvalidOperationException: parser failed")
        })
        {
            try { OriginalCorpusChecks.CheckMessages([.. baseline, unexpected], plugin); }
            catch (Exception error) when (error.Message.Contains("Unexpected original parser message: " + unexpected,
                StringComparison.Ordinal)) { continue; }
            throw new Exception("Corpus diagnostics accepted an unknown message or lost its original text.");
        }
        Console.WriteLine("Corpus diagnostics accept the exact deployment rename and retain unknown-message failures.");
    }
}
