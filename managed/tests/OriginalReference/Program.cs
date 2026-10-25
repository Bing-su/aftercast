using System.Reflection;
using System.Runtime.Loader;
using System.Security.Cryptography;
using System.Text.Json;

// Probe explicit data contracts and configured actions without invoking ACT's entry point or main form.
internal static class Program
{
    private const string ExpectedActHash = "934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657";
    private static int failures;

    private static void Probe(string stage, Func<object> action)
    {
        try { Console.WriteLine(JsonSerializer.Serialize(new { stage, success = true, result = action() })); }
        catch (Exception error)
        {
            failures++;
            // Preserve the actual loader/initializer failure instead of the reflection invocation wrapper.
            while (error is TargetInvocationException && error.InnerException is not null) error = error.InnerException;
            Console.WriteLine(JsonSerializer.Serialize(new { stage, success = false, error = error.ToString() }));
        }
    }

    public static int Main(string[] args)
    {
        if (args.Length is < 2 or > 5) throw new ArgumentException("Pass the original ACT EXE path, headless Forms DLL directory and optional mode");
        var actPath = Path.GetFullPath(args[0]);
        var formsDirectory = Path.GetFullPath(args[1]);
        var hash = Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(actPath)));
        if (hash != ExpectedActHash) throw new InvalidOperationException("Original ACT artifact hash does not match the researched version");
        if (args.Length >= 3 && args[2] is "audio" or "text-exports" or "timer-models" or "eq2-timestamps" or "benchmark-ui" or "plugin-metadata" or "plugin-updates" or "parent-settings")
            AssemblyLoadContext.Default.LoadFromAssemblyName(new AssemblyName("System.Windows.Forms"));
        else
            foreach (var name in new[] { "System.Windows.Forms.dll", "System.Windows.Forms.DataVisualization.dll" })
                AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.Combine(formsDirectory, name));
        var act = AssemblyLoadContext.Default.LoadFromAssemblyPath(actPath);
        Console.WriteLine(JsonSerializer.Serialize(new { assembly = act.FullName, sha256 = hash, source = act.Location }));
        Probe("Original default localization", () =>
        {
            // Install ACT's own shipped string tables without creating its localization UI or main form.
            var localization = act.GetType("Advanced_Combat_Tracker.ActGlobals+ActLocalization", throwOnError: true)!;
            localization.GetMethod("Init", BindingFlags.Static | BindingFlags.NonPublic)!.Invoke(null, null);
            localization.GetMethod("AddPrebuild", BindingFlags.Static | BindingFlags.NonPublic)!.Invoke(null, null);
            var strings = (System.Collections.IDictionary)localization.GetField("strings", BindingFlags.Static | BindingFlags.NonPublic)!.GetValue(null)!;
            var globals = act.GetType("Advanced_Combat_Tracker.ActGlobals", throwOnError: true)!;
            return new { entries = strings.Count, mainFormIsNull = globals.GetField("oFormActMain")!.GetValue(null) is null };
        });
        if (args.Length >= 3)
        {
            var unexpectedFailures = args[2] switch
            {
                "values" when args.Length == 3 => DnumValues.Run(act),
                "actions" when args.Length == 3 => AggregateValues.Run(act),
                "contracts" when args.Length == 4 => RunContracts(args[3]),
                "recorded-actions" when args.Length == 5 => RunRecordedActions(args[3], args[4]),
                "text-exports" when args.Length == 4 => RunReferenceProbe(args[3], "TextExportProbe", "Original text exports"),
                "timer-models" when args.Length == 4 => RunReferenceProbe(args[3], "TimerModelProbe", "Original timer models"),
                "audio" when args.Length == 4 => RunReferenceProbe(args[3], "AudioProbe", "Original audio"),
                "parent-settings" when args.Length == 4 => RunReferenceProbe(args[3], "ParentSettingsProbe", "Original parent settings"),
                "selective-parsing" when args.Length == 4 => RunReferenceProbe(args[3], "SelectiveParsingProbe", "Original selective parsing"),
                "custom-trigger-models" when args.Length == 4 => RunReferenceProbe(args[3], "CustomTriggerProbe", "Original custom trigger models"),
                "data-corrections" when args.Length == 4 => RunReferenceProbe(args[3], "DataCorrectionProbe", "Original data corrections"),
                "encounter-xml" when args.Length == 4 => RunReferenceProbe(args[3], "EncounterXmlProbe", "Original encounter XML"),
                "encounter-xml-import" when args.Length == 4 => RunReferenceProbe(args[3], "EncounterXmlProbe", "Original encounter XML import", "RunImport"),
                "history-records" when args.Length == 4 => RunReferenceProbe(args[3], "HistoryRecordProbe", "Original history records"),
                "eq2-timestamps" when args.Length == 4 => RunReferenceProbe(args[3], "Eq2TimestampProbe", "Original EQ2 timestamps"),
                "benchmark-ui" when args.Length == 4 => RunReferenceProbe(args[3], "BenchmarkUiProbe", "Original Benchmark UI"),
                "plugin-metadata" when args.Length == 4 => RunReferenceProbe(args[3], "PluginMetadataProbe", "Original plugin metadata"),
                "plugin-updates" when args.Length == 4 => RunReferenceProbe(args[3], "PluginUpdateProbe", "Original plugin update query"),
                _ => throw new ArgumentException("Unknown original-reference mode")
            };
            return failures + unexpectedFailures == 0 ? 0 : 1;
        }
        Type Type(string name) => act.GetType("Advanced_Combat_Tracker." + name, throwOnError: true)!;
        object Create(string name, params object?[] arguments) => Activator.CreateInstance(Type(name), arguments)!;
        static object? Read(object instance, string name) => instance.GetType().GetProperty(name)!.GetValue(instance);
        object? number = null, zone = null, encounter = null;
        Probe("Dnum constructor", () =>
        {
            number = Create("Dnum", 1234567890123L);
            return new { number = Read(number, "Number"), assembly = number.GetType().Assembly.Location };
        });
        if (number is not null) Probe("Dnum formatting", () => new { text = number.ToString() });
        Probe("ZoneData constructor", () =>
        {
            zone = Create("ZoneData", DateTime.UnixEpoch, "Fixture Zone", false, false, false);
            return new { name = Read(zone, "ZoneName"), start = Read(zone, "StartTime") };
        });
        if (zone is not null) Probe("EncounterData constructor", () =>
        {
            encounter = Create("EncounterData", "Fixture Player", "Fixture Zone", false, zone);
            return new { character = Read(encounter, "CharName"), parentMatches = ReferenceEquals(zone, Read(encounter, "Parent")) };
        });
        if (encounter is not null) Probe("CombatantData constructor", () =>
        {
            var combatant = Create("CombatantData", "Fixture Player", encounter);
            return new { name = Read(combatant, "Name"), parentMatches = ReferenceEquals(encounter, Read(combatant, "Parent")) };
        });
        if (encounter is not null) Probe("EncounterData metadata getters", () =>
            new { zone = Read(encounter, "ZoneName"), title = Read(encounter, "Title") });
        Console.WriteLine(JsonSerializer.Serialize(new { failures }));
        return failures == 0 ? 0 : 1;
    }

    private static int RunRecordedActions(string probePath, string transcriptPath)
    {
        var probes = AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.GetFullPath(probePath));
        Probe("Original recorded actions", () => probes.GetType("Aftercast.LegacyReference.RecordedActionProbe", throwOnError: true)!
            .GetMethod("Run", BindingFlags.Public | BindingFlags.Static)!.Invoke(null, [Path.GetFullPath(transcriptPath)])!);
        return 0;
    }
    private static int RunReferenceProbe(string probePath, string probe, string stage, string method = "Run")
    {
        var probes = AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.GetFullPath(probePath));
        Probe(stage, () => probes.GetType("Aftercast.LegacyReference." + probe, true)!
            .GetMethod(method)!.Invoke(null, null)!);
        return 0;
    }

    private static int RunContracts(string path)
    {
        var probes = AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.GetFullPath(path));
        foreach (var name in new[] { "DefaultGlobalsProbe", "DnumValueProbe", "CombatStateProbe", "DamageStateProbe", "ChildActionProbe", "CombatActionProbe", "CombatEventProbe", "AutomaticAlliesProbe", "EncounterFinalizationProbe", "CombatDurationProbe", "AggregateQueryProbe", "HitRateProbe" })
            Probe("Original " + name, () => probes.GetType("Aftercast.LegacyReference." + name, throwOnError: true)!
                .GetMethod("Run", BindingFlags.Public | BindingFlags.Static)!.Invoke(null, null)!);
        return 0;
    }
}
