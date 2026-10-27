using System.Reflection;
using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;
using System.Runtime.ExceptionServices;
using System.Runtime.InteropServices;
using System.Runtime.Loader;
using System.Security.Cryptography;
using System.Text;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Bridge;

internal static class Program
{
    private sealed record ImportedAction(bool Import, string Attacker, string Victim, string Attack,
        int Swing, long Damage, DateTime Time, int Sorter);
    private static void Check(bool condition, [System.Runtime.CompilerServices.CallerArgumentExpression(nameof(condition))] string? expression = null)
    {
        if (!condition) throw new Exception("Managed contract check failed: " + expression);
    }
    internal static string Diagnostic()
    {
        var output = Marshal.AllocHGlobal(8192);
        try
        {
            var length = EntryPoint.ReadError(output, 8192);
            Check(length >= 0 && length <= 8192);
            return Marshal.PtrToStringUTF8(output, length)!;
        }
        finally { Marshal.FreeHGlobal(output); }
    }
    [STAThread]
    public static void Main(string[] args)
    {
        var mode = args[0];
        // Only the original-reference test calls ACT UI types before startup; ABI modes exercise normal binding.
        if (OperatingSystem.IsLinux() && mode is "act" or "recorded-actions" or "selective-parsing" or "custom-trigger-models" or "data-corrections" or "encounter-xml" or "history-records")
        {
            AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.Combine(AppContext.BaseDirectory, "System.Windows.Forms.dll"));
            AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.Combine(AppContext.BaseDirectory, "System.Windows.Forms.DataVisualization.dll"));
        }
        // Modes keep artifact-dependent checks separate from ordinary ABI tests without a test framework dependency.
        if (mode is "contracts" or "query-load" or "archive-load" or "original" or "original-import" or "original-corpus" or "original-log" or "text-exports" or "timer-models" or "eq2-timestamps" or "parent-settings") Platform.Prepare();
        switch (mode)
        {
            case "bridge": Bridge(); break;
            case "contracts": Contracts(); break;
            case "log-load": LogLoadChecks.Run(); break;
            case "query-load": CombatQueryLoadChecks.Run(); break;
            case "archive-load": ArchiveLoadChecks.Run(); break;
            case "plugins": PluginRequests(); break;
            case "prepare-act": PrepareAct(); break;
            case "act": ActBinding(); break;
            case "linux": LinuxCompatibility(); break;
            case "original": ObserveOriginal(args[1], args[2], args.Length > 3 ? args[3] : null); break;
            case "original-import": ObserveOriginal(args[1], args[2], "managed/tests/fixtures/ffxiv-synthetic.log"); break;
            case "original-corpus": ObserveOriginal(args[1], args[2], args[3], fixedFixture: false); break;
            case "original-log": ObserveOriginal(args[1], args[2], args[3], fixedFixture: false, localLog: true); break;
            case "recorded-actions": RecordedActions(args[1]); break;
            case "external-plugin": ExternalPluginChecks.Run(args[1], args[2], args[3]); break;
            case "overlay-stack": OverlayStackChecks.Run(args[1], args[2], args[3], args[4]); return;
            case "overlay-integration": OverlayStackChecks.Run(args[1], args[2], args[3], args[4], OverlayIntegrationChecks.Run); return;
            case "plugin-metadata": ReferenceContract("PluginMetadataProbe", "Plugin metadata"); break;
            case "plugin-updates": ReferenceContract("PluginUpdateProbe", "Plugin update query"); break;
            case "overlay-renderer": OverlayRendererChecks.Run(args[1]); break;
            case "text-exports": ReferenceContract("TextExportProbe", "Text exports"); break;
            case "timer-models": ReferenceContract("TimerModelProbe", "Timer models"); break;
            case "audio": ReferenceContract("AudioProbe", "Audio"); break;
            case "parent-settings": ReferenceContract("ParentSettingsProbe", "Parent settings"); break;
            case "selective-parsing": ReferenceContract("SelectiveParsingProbe", "Selective parsing"); break;
            case "custom-trigger-models": ReferenceContract("CustomTriggerProbe", "Custom trigger models"); break;
            case "data-corrections": ReferenceContract("DataCorrectionProbe", "Data corrections"); break;
            case "encounter-xml": ReferenceContract("EncounterXmlProbe", "Encounter XML export"); break;
            case "encounter-xml-import": Platform.Prepare(); ReferenceContract("EncounterXmlProbe", "Encounter XML import", "RunImport"); break;
            case "history-records": ReferenceContract("HistoryRecordProbe", "History record policies"); break;
            case "eq2-timestamps": ReferenceContract("Eq2TimestampProbe", "EQ2 timestamps"); break;
            case "benchmark-ui": ReferenceContract("BenchmarkUiProbe", "Benchmark UI"); break;
            case "external-benchmark": ExternalPluginChecks.Run(args[1], "ACT_Plugin.EventHandlerBenchmark", args[2], ExternalBenchmarkChecks.Run); break;
            case "external-export": ExternalPluginChecks.Run(args[1], "ACT_Plugin1.Export_All", args[2], (_, _) => ExternalExportChecks.Run()); break;
            case "external-timer": ExternalPluginChecks.Run(args[1], "ACT_Plugin.TimerRepeat", args[2], ExternalTimerChecks.Run); break;
            case "external-eq2": ExternalPluginChecks.Run(args[1], "ACT_Plugin.ACT_English_Parser", args[2], ExternalEq2Checks.Run); break;
            default: throw new ArgumentException("Unknown managed test mode");
        }
        Console.WriteLine($"Managed {args[0]} contract checks passed.");
    }
    private static void RecordedActions(string path)
    {
        var actual = typeof(Dnum).Assembly;
        Check(SHA256.HashData(File.ReadAllBytes(actual.Location)).AsSpan().SequenceEqual(
            SHA256.HashData(File.ReadAllBytes("crates/aftercast/managed/Advanced Combat Tracker.dll"))));
        var legacy = Assembly.LoadFrom(Path.GetFullPath("managed/tests/LegacyReference/bin/Release/net10.0/Aftercast.LegacyReference.dll"));
        try { Console.WriteLine(legacy.GetType("Aftercast.LegacyReference.RecordedActionProbe")!.GetMethod("Run")!.Invoke(null, [path])); }
        catch (TargetInvocationException error) when (error.InnerException is not null)
        { ExceptionDispatchInfo.Capture(error.InnerException).Throw(); throw; }
    }
    private static void ReferenceContract(string probe, string expected, string method = "Run")
    {
        var probes = Assembly.LoadFrom(Path.GetFullPath("managed/tests/LegacyReference/bin/Release/net10.0/Aftercast.LegacyReference.dll"));
        try { Check((string)probes.GetType("Aftercast.LegacyReference." + probe)!.GetMethod(method)!.Invoke(null, null)! == expected); }
        catch (TargetInvocationException error) when (error.InnerException is not null)
        { ExceptionDispatchInfo.Capture(error.InnerException).Throw(); throw; }
    }

    private static void ObserveOriginal(string pluginPath, string statePath, string? importPath,
        bool fixedFixture = true, bool localLog = false)
    {
        // Run only an explicitly supplied trusted copy; wait long enough to observe caught worker failures.
        var session = new PluginSession();
        Directory.CreateDirectory(statePath);
        session.Configure(Path.GetFullPath(statePath));
        var path = Path.GetFullPath(pluginPath);
        var instance = session.Construct(path, "FFXIV_ACT_Plugin.FFXIV_ACT_Plugin");
        var id = session.Add(path, instance);
        try
        {
            Console.WriteLine("Original status: " + session.Initialize(id));
            if (OperatingSystem.IsLinux())
            {
                // Verify the actual plugin binding, not only the standalone byte-rewrite helper.
                var logfile = AssemblyLoadContext.GetLoadContext(instance.GetType().Assembly)!.Assemblies
                    .Single(assembly => assembly.GetName().Name == "FFXIV_ACT_Plugin.Logfile");
                var run = logfile.GetType("FFXIV_ACT_Plugin.Logfile.LogOutput")!
                    .GetMethod("Run", BindingFlags.NonPublic | BindingFlags.Public | BindingFlags.Instance)!;
                Check(logfile.Location == "" && Convert.ToHexStringLower(SHA256.HashData(run.GetMethodBody()!.GetILAsByteArray()!)) ==
                    "c37628922371e34b053e65a3f21ea456c9c7e502bc879d139332b6f68dae8ce8");
                var resource = AssemblyLoadContext.GetLoadContext(instance.GetType().Assembly)!.Assemblies
                    .Single(assembly => assembly.GetName().Name == "FFXIV_ACT_Plugin.Resource");
                var load = resource.GetType("FFXIV_ACT_Plugin.Resource.ResourceFile")!
                    .GetMethod("LoadResourceDictionary", BindingFlags.NonPublic | BindingFlags.Instance)!;
                Check(resource.Location == "" && Convert.ToHexStringLower(SHA256.HashData(load.GetMethodBody()!.GetILAsByteArray()!)) ==
                    "f4f48266e0b23c7d61c12cd3cf1b10c02d2dc904f33b4e1230b54cfe8bf2d2ba");
            }
            var elapsed = System.Diagnostics.Stopwatch.StartNew();
            while (elapsed.Elapsed < TimeSpan.FromMilliseconds(1500)) { session.Pump(); Thread.Sleep(10); }
            var root = ActGlobals.oFormActMain!;
            object Field(object owner, string name) => owner.GetType().GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(owner)!;
            var settingsPage = Field(Field(instance, "_actUIMods"), "_settingsPropertyPage");
            var list = Field(settingsPage, "lstMessages");
            var initialMessages = ((System.Collections.IEnumerable)list.GetType().GetProperty("Items")!.GetValue(list)!)
                .Cast<object>().Select(item => item.ToString()!).ToArray();
            using var parserCapture = new OriginalParserMessageCapture(Field(settingsPage, "_settingsManager"), initialMessages);
            if (importPath is not null && !fixedFixture)
            {
                // Pin source images before attaching provenance to a replay, including the actual Costura Parse resource.
                var plugin = instance.GetType().Assembly;
                var mainHash = Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(path)));
                var expectedParseHash = mainHash switch
                {
                    "af1031cdba8eeba8f53dd9d618e58d62d4919bfa77db9b31c568bd04cb359093" =>
                        "aabdeedb35f0e9fb2a833cc55e11cf9f7684ca06cddbaffa2a791754676afea9",
                    "f7f0c8894ba1a152b6a453427773496f5948cf412171da773578d8671f643a81" when OperatingSystem.IsWindows() =>
                        "efd2db36df247eb23e0a79dd3d19fb2a970f1b52a48901b722fd07728a2aac4a",
                    _ => throw new InvalidOperationException("Unsupported original plugin image for corpus verification")
                };
                using var resource = plugin.GetManifestResourceStream("costura.ffxiv_act_plugin.parse.dll.compressed")!;
                using var parse = new System.IO.Compression.DeflateStream(resource, System.IO.Compression.CompressionMode.Decompress);
                var parseHash = Convert.ToHexStringLower(SHA256.HashData(parse));
                Check(parseHash == expectedParseHash);
                using var input = File.OpenRead(importPath);
                OriginalCorpusChecks.Run(session, importPath, new(
                    Convert.ToHexStringLower(SHA256.HashData(input)), mainHash, parseHash,
                    "934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657"), localLog);
            }
            if (importPath is not null && fixedFixture)
            {
                Check(Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(importPath))) ==
                    "05e458eaa5f376db6424042a8d16ed1ed28b4e99ab3c14b0d4339fa013e3f35e");
                var actions = new List<ImportedAction>();
                var live = (root.ActiveZone, root.CurrentZone, root.LogFilePath, root.LastKnownTime, ActGlobals.charName);
                Check(live.charName == "YOU");
                var subscription = session.SubscribeLogs(2, 8 * 1024 * 1024);
                root.AfterCombatAction += (import, info) =>
                {
                    lock (actions) actions.Add(new(import, info.attacker, info.victim,
                        info.theAttackType, info.swingType, info.damage.Number, info.time, info.timeSorter));
                };
                for (var iteration = 0; iteration < 3; iteration++)
                {
                    // Reset state between repeated imports and recover after a source failure without inventing an EOF event.
                    if (iteration == 2)
                    {
                        var invalid = Path.GetFullPath(Path.Combine(statePath, "invalid-utf8.log"));
                        File.WriteAllBytes(invalid, [0xff]);
                        try { session.ImportLog(invalid); throw new Exception("Invalid original-import source succeeded"); }
                        catch (InvalidDataException) { }
                    }
                    lock (actions) actions.Clear();
                    var report = session.ImportLog(Path.GetFullPath(importPath));
                    var logs = session.DrainLogs(subscription, 8192);
                    Check(logs.Records == 2 && logs.Dropped == 5 && logs.RequiredPayloadCapacity == 0);
                    Check(logs.FirstDropped == (ulong)(iteration * 7 + 1) && logs.LastDropped == (ulong)(iteration * 7 + 5));
                    Console.WriteLine("Original import summary: " + System.Text.Json.JsonSerializer.Serialize(report));
                    Check(report == new ImportResult(7, 7, 1, 2, 400, 0));
                    var snapshot = session.ReadCombatSnapshot(0)!;
                    Check(snapshot.IsImport && snapshot.Status == CombatSnapshotStatus.Ended);
                    Check(snapshot.Damage == 400 && snapshot.Healed == 0 && snapshot.Combatants.Length == 2);
                    Check(snapshot.Zone == "Limsa Lominsa Lower Decks");
                    Check(snapshot.DurationTicks == TimeSpan.FromSeconds(2).Ticks);
                    var player = snapshot.Combatants.Single(actor => actor.Name == "YOU");
                    var enemy = snapshot.Combatants.Single(actor => actor.Name == "Fixture Enemy");
                    Check(player.IsAlly && !enemy.IsAlly && player.Damage == 400 && enemy.DamageTaken == 400);
                    Check(player.Hits == 2 && player.Swings == 2 && player.ToHit == 100 && player.EncDPS == 200);
                    Check(session.ReadCombatSnapshot(snapshot.Revision) is null);
                    Console.WriteLine("Original combat snapshot: " + snapshot.Title + "; damage=" + snapshot.Damage + "; revision=" + snapshot.Revision);
                    Check(!root.InCombat && ReferenceEquals(root.ActiveZone, live.ActiveZone));
                    Check(root.CurrentZone == live.CurrentZone && root.LogFilePath == live.LogFilePath);
                    Check(root.LastKnownTime == live.LastKnownTime && ActGlobals.charName == live.charName);
                    var imported = root.ZoneList[0].Items[^1];
                    // Exercise the original plugin's actual callback chain through the restored ACT base column.
                    var expectedHitRate = 100f.ToString(ActGlobals.mainTableShowCommas ? "#,0.00" : "0.00");
                    Check(imported.GetCombatant("Fixture Enemy")!.GetColumnByName("IncToHit") == expectedHitRate);
                    lock (actions)
                    {
                        Console.WriteLine("Original imported actions: " + System.Text.Json.JsonSerializer.Serialize(actions));
                        Check(actions.Count == 2);
                        for (var index = 0; index < actions.Count; index++)
                        {
                            var action = actions[index];
                            Check(action.Import && action.Attacker == "YOU" && action.Victim == "Fixture Enemy");
                            Check(action.Attack == "attack" && action.Swing == 0 && action.Damage == (index == 0 ? 100 : 300));
                            Check(action.Time.ToUniversalTime() == new DateTime(2026, 1, 1, 0, 0, index == 0 ? 1 : 3, DateTimeKind.Utc));
                        }
                        Check(actions[1].Sorter == actions[0].Sorter + 1);
                    }
                }
                session.UnsubscribeLogs(subscription);
            }
            var errors = (Tuple<long, string>)root.GetType().GetProperty("Errors", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(root)!;
            Console.WriteLine($"Original worker diagnostics: {errors.Item1}; {errors.Item2}");
            foreach (var diagnostic in (string[])root.GetType().GetProperty("RecentDiagnostics", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(root)!)
                Console.WriteLine(diagnostic);
            var parserMessages = parserCapture.Snapshot();
            foreach (var message in parserMessages) Console.WriteLine("Original parser message: " + message);
            OriginalCorpusChecks.CheckMessages(parserMessages, path,
                instance.GetType().Assembly.GetName().Version == new Version(3, 0, 3, 4) ? "1.5.1" : "1.5.0",
                allowSimulatedDamageWarnings: !fixedFixture && !localLog,
                recordedMessages: localLog ? OriginalCorpusChecks.RecordedMessages(importPath!, line => root.GetDateTimeFromLog(line)) : null,
                requireStartupMessages: OperatingSystem.IsLinux());
            Check(errors.Item1 == 0);
        }
        finally
        {
            Console.WriteLine("Original shutdown: stopping");
            session.Stop();
            Console.WriteLine("Original shutdown: stopped");
        }
    }
    private static void Bridge()
    {
        var buffer = Marshal.AllocHGlobal(24);
        try
        {
            Check(EntryPoint.Invoke(0, 24) == 1);
            Check(EntryPoint.Invoke(buffer, 23) == 1);
            Check(EntryPoint.ConfigureHost(0, 40) == -1);
            Check(EntryPoint.CreatePlugin(0, 40) == -1);
            Check(EntryPoint.PluginCommand(0, 8224) == -1);
            Marshal.WriteInt32(buffer, 0, 24);
            Marshal.WriteInt32(buffer, 4, 2);
            Check(EntryPoint.Invoke(buffer, 24) == 2);
            Marshal.WriteInt32(buffer, 4, 1);
            Marshal.WriteInt32(buffer, 8, 99);
            Marshal.WriteInt64(buffer, 16, 0);
            Check(EntryPoint.Invoke(buffer, 24) == 6);
            Check(EntryPoint.ReadError(0, 0) == -1);
            var expected = "Unknown bridge command: 99";
            var required = EntryPoint.ReadError(buffer, 1);
            Check(required == Encoding.UTF8.GetByteCount(expected));
            Check(Marshal.ReadInt32(buffer, 0) == 24);
            Check(Diagnostic() == expected);
            Marshal.WriteInt32(buffer, 8, 2);
            Check(EntryPoint.Invoke(buffer, 24) == 4);
            Marshal.WriteInt32(buffer, 8, 1);
            Check(EntryPoint.Invoke(buffer, 24) == 0);
            Check(EntryPoint.ReadError(buffer, 0) == 0);
            var session = Marshal.ReadInt64(buffer, 16);
            Check(session > 0 && Marshal.ReadInt32(buffer, 12) == 10);
            DiagnosticAbiChecks.Run(session);
            SpeechChecks.Abi(session);
            ImportAbiChecks.Run(session);
            ArchiveAbiChecks.Run(session);
            LogAbiChecks.Run(session);
            CombatSnapshotAbiChecks.Run(session);
            CombatQueryAbiChecks.Run(session);
            EndCombatAbiChecks.Run(session);
            ReentrantSessionChecks.Run(session);
            Marshal.WriteInt32(buffer, 8, 4);
            Check(EntryPoint.Invoke(buffer, 24) == 0 && Marshal.ReadInt32(buffer, 12) == 0);
            Marshal.WriteInt32(buffer, 8, 1);
            Check(EntryPoint.Invoke(buffer, 24) == 3);
            Marshal.WriteInt32(buffer, 8, 3);
            Marshal.WriteInt64(buffer, 16, session + 1);
            Check(EntryPoint.Invoke(buffer, 24) == 5);
            Marshal.WriteInt64(buffer, 16, session);
            Check(EntryPoint.Invoke(buffer, 24) == 0);
            Check(EntryPoint.Invoke(buffer, 24) == 4);
            // Neither runtime assembly may silently regain a compiler runtime dependency.
            foreach (var assembly in new[] { typeof(EntryPoint).Assembly, typeof(Dnum).Assembly })
                Check(assembly.GetReferencedAssemblies().All(reference => reference.Name != "FSharp.Core"));
        }
        finally { Marshal.FreeHGlobal(buffer); }
    }
    private static void PluginRequests()
    {
        var memory = Marshal.AllocHGlobal(65537);
        var directory = Directory.CreateTempSubdirectory("aftercast-plugin-requests-");
        try
        {
            Marshal.WriteInt32(memory, 0, 24);
            Marshal.WriteInt32(memory, 4, 1);
            Marshal.WriteInt32(memory, 8, 1);
            Check(EntryPoint.Invoke(memory, 24) == 0);
            var session = Marshal.ReadInt64(memory, 16);
            var path = Encoding.UTF8.GetBytes(Path.GetFullPath("managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll"));
            var name = Encoding.UTF8.GetBytes("Aftercast.PluginFixture.GoodPlugin");
            var size = 40 + path.Length + name.Length;
            void Reset()
            {
                Marshal.WriteInt32(memory, 0, size);
                Marshal.WriteInt32(memory, 4, 1);
                Marshal.WriteInt64(memory, 8, session);
                Marshal.WriteInt32(memory, 16, 40);
                Marshal.WriteInt32(memory, 20, path.Length);
                Marshal.WriteInt32(memory, 24, 40 + path.Length);
                Marshal.WriteInt32(memory, 28, name.Length);
                Marshal.WriteInt64(memory, 32, 0);
                Marshal.Copy(path, 0, memory + 40, path.Length);
                Marshal.Copy(name, 0, memory + 40 + path.Length, name.Length);
            }
            void Rejected(Action change, string expected)
            {
                Reset();
                change();
                Check(EntryPoint.CreatePlugin(memory, size) == -1);
                Check(Diagnostic().Contains(expected));
                Check(Marshal.ReadInt64(memory, 32) == 0);
            }
            Check(EntryPoint.CreatePlugin(0, 40) == -1);
            Check(EntryPoint.CreatePlugin(memory, 39) == -1);
            Check(EntryPoint.CreatePlugin(memory, 65537) == -1);
            Rejected(() => Marshal.WriteInt32(memory, 4, 2), "size/version");
            Rejected(() => Marshal.WriteInt64(memory, 8, session + 1), "does not own");
            Rejected(() => Marshal.WriteInt32(memory, 16, int.MaxValue), "outside");
            Rejected(() => Marshal.WriteInt32(memory, 20, int.MaxValue), "outside");
            Rejected(() => Marshal.WriteInt32(memory, 28, -1), "outside");
            Rejected(() => Marshal.WriteInt32(memory, 24, 0), "outside");
            Rejected(() => Marshal.WriteByte(memory, 40, 0), "NUL");
            Rejected(() => Marshal.WriteByte(memory, 40, 255), "translate");
            // Production callers configure state before constructing plugins; the ABI fixture must do the same.
            var shared = ((Lazy<PluginSession>)typeof(EntryPoint)
                .GetField("plugins", BindingFlags.NonPublic | BindingFlags.Static)!.GetValue(null)!).Value;
            shared.Configure(directory.FullName);
            Reset();
            Check(EntryPoint.CreatePlugin(memory, size) == 0);
            var instance = Marshal.ReadInt64(memory, 32);
            Check(instance > 0 && Diagnostic() == "");
            void Command()
            {
                Marshal.WriteInt32(memory, 0, 8224);
                Marshal.WriteInt32(memory, 4, 1);
                Marshal.WriteInt64(memory, 8, session);
                Marshal.WriteInt64(memory, 16, instance);
                Marshal.WriteInt32(memory, 24, 99);
                Marshal.WriteInt32(memory, 28, 0);
            }
            Check(EntryPoint.PluginCommand(0, 8224) == -1);
            Check(EntryPoint.PluginCommand(memory, 8223) == -1);
            Check(EntryPoint.PluginCommand(memory, 8225) == -1);
            Command();
            Marshal.WriteInt32(memory, 4, 2);
            Check(EntryPoint.PluginCommand(memory, 8224) == -1 && Diagnostic().Contains("size/version"));
            Command();
            Marshal.WriteInt64(memory, 8, session + 1);
            Check(EntryPoint.PluginCommand(memory, 8224) == -1 && Diagnostic().Contains("does not own"));
            Command();
            Check(EntryPoint.PluginCommand(memory, 8224) == -1 && Diagnostic().Contains("Unknown plugin lifecycle command"));
            Command();
            Marshal.WriteInt32(memory, 24, 1);
            Marshal.WriteInt64(memory, 16, long.MaxValue);
            Check(EntryPoint.PluginCommand(memory, 8224) == -1 && Diagnostic().Contains("Unknown plugin instance"));
            Marshal.WriteInt32(memory, 0, 24);
            Marshal.WriteInt32(memory, 4, 1);
            Marshal.WriteInt32(memory, 8, 3);
            Marshal.WriteInt64(memory, 16, session);
            Check(EntryPoint.Invoke(memory, 24) == 0);
            Reset();
            Check(EntryPoint.CreatePlugin(memory, size) == -1 && Diagnostic().Contains("No managed ACT session"));
        }
        finally { Marshal.FreeHGlobal(memory); directory.Delete(true); }
    }
    private static void PrepareAct()
    {
        using var input = File.OpenRead("binaries/act/Advanced Combat Tracker.exe");
        Check(Convert.ToHexStringLower(SHA256.HashData(input)) == "934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657");
        input.Position = 0;
        using var pe = new PEReader(input);
        var metadata = pe.GetMetadataReader();
        var assembly = metadata.GetAssemblyDefinition();
        Check(metadata.GetString(assembly.Name) == "Advanced Combat Tracker" && assembly.Version == new Version(3, 8, 5, 288));
        Check(File.ReadAllBytes("managed/ActCompat/ACT.publickey").AsSpan().SequenceEqual(metadata.GetBlobBytes(assembly.PublicKey)));
    }
    private static void ActBinding()
    {
        // Load only the compiled original-reference probe; never execute the supplied ACT EXE.
        var legacy = Assembly.LoadFrom(Path.GetFullPath("managed/tests/LegacyReference/bin/Release/net10.0/Aftercast.LegacyReference.dll"));
        const string identity = "Advanced Combat Tracker, Version=3.8.5.288, Culture=neutral, PublicKeyToken=a946b61e93d97868";
        Check(legacy.GetReferencedAssemblies().Single(item => item.Name == "Advanced Combat Tracker").FullName == identity);
        var actual = typeof(Dnum).Assembly;
        Check(actual.FullName == identity);
        Check(SHA256.HashData(File.ReadAllBytes(actual.Location)).AsSpan().SequenceEqual(
            SHA256.HashData(File.ReadAllBytes("crates/aftercast/managed/Advanced Combat Tracker.dll"))));
        Check(!typeof(Dnum).IsValueType && !typeof(Dnum).IsSealed);
        object? Call(string type, string name, params object[] arguments)
        {
            try { return legacy.GetType("Aftercast.LegacyReference." + type)!.GetMethod(name)!.Invoke(null, arguments); }
            catch (TargetInvocationException error) when (error.InnerException is not null)
            { ExceptionDispatchInfo.Capture(error.InnerException).Throw(); throw; }
        }
        Check((string)Call("DefaultGlobalsProbe", "Run")! == "Default globals");
        Check((long)Call("ContractProbe", "Run")! == 123);
        Check((string)Call("DnumValueProbe", "Run")! == "Dnum values");
        // These probes assert Linux model behavior; native Windows controls have separate fixture checks.
        if (OperatingSystem.IsLinux())
        {
            Check((string)Call("ChartProbe", "Run")! == "Network");
            Check((string)Call("ListViewProbe", "Run")! == "42");
            Check((int)Call("TimerProbe", "Run")! == 1);
            Check((int)Call("ButtonProbe", "Run")! == 2);
            Check((int)Call("ControlModelProbe", "Run")! == 200);
            Check((int)Call("FormStateProbe", "Run")! == 2);
            Check((int)Call("SettingsControlProbe", "Run")! == 2);
        }
        Check((string)Call("RegistrationProbe", "Run")! == "changed callback");
        Check((string)Call("TreeViewProbe", "Run")! == "Tree nodes");
        Check((string)Call("CombatStateProbe", "Run")! == "Combat state");
        Check((string)Call("DamageStateProbe", "Run")! == "Fixture attack");
        Check((string)Call("ChildActionProbe", "Run")! == "Child actions");
        Check((string)Call("CombatActionProbe", "Run")! == "Combat actions");
        Check((string)Call("CombatEventProbe", "Run")! == "Combat events");
        Check((string)Call("AutomaticAlliesProbe", "Run")! == "Automatic allies");
        Check((string)Call("EncounterFinalizationProbe", "Run")! == "Encounter finalization");
        Check((string)Call("CombatDurationProbe", "Run")! == "Combat durations");
        Check((string)Call("AggregateQueryProbe", "Run")! == "Aggregate queries");
        Check((string)Call("HitRateProbe", "Run")! == "Hit rates");
        Check(typeof(Dnum).GetProperty("Number")!.GetSetMethod() is null && new Dnum(-11).Number == -11);
        Check(new Dnum(1).ToString() == "1");
        var directory = Directory.CreateTempSubdirectory("aftercast-contract-");
        try
        {
            Check(Call("HostProbe", "Root") is null);
            var previous = new HostEnvironment(directory.FullName);
            previous.Dispose();
            using (var current = new HostEnvironment(directory.FullName))
            {
                var root = ActGlobals.oFormActMain;
                previous.Dispose();
                Check(ReferenceEquals(root, ActGlobals.oFormActMain));
            }
            using (var environment = new HostEnvironment(directory.FullName))
            {
                Check(Call("HostProbe", "Root") is not null);
                Check((string)Call("StartupProbe", "Run")! == "original");
                Check((string)Call("LogHostProbe", "Run")! == "Network_*.log");
                Check((int)Call("MasterSwingProbe", "Run")! == 42);
                var main = ActGlobals.oFormActMain!;
                CheckRegistrationViews(main);
                var payload = new LogLineEventArgs("initial", 1, DateTime.UnixEpoch, "zone", false);
                var calls = 0;
                LogLineEventDelegate line = (import, info) => { Check(import && ReferenceEquals(info, payload)); info.logLine = "updated"; calls++; };
                main.BeforeLogLineRead += line;
                main.GetType().GetMethod("DispatchLogLine", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(main, [true, payload]);
                main.BeforeLogLineRead -= line;
                main.GetType().GetMethod("DispatchLogLine", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(main, [true, payload]);
                Check(calls == 1 && payload.logLine == "updated" && payload.originalLogLine == "initial");
                Check((string)Call("HostProbe", "Character", "Example Character")! == "Example Character");
                Check((string)Call("HostProbe", "AppDataPath")! == directory.FullName);
                Check((bool)Call("HostProbe", "MissingRegistration")!);
                Call("HostProbe", "BackgroundDiagnostic");
                var (count, diagnostic) = environment.Diagnostics;
                Check(count == 1 && diagnostic.Contains("background diagnostic") && environment.Errors.Item1 == 0);
                Check((string)Call("SettingsSerializerProbe", "Run")! == "after");
                Check(environment.Diagnostics.Item1 == 3 && environment.Errors.Item1 == 2);
                try { Call("HostProbe", "Restart"); throw new Exception("Embedded restart succeeded"); }
                catch (PlatformNotSupportedException) { }
            }
            Check(Call("HostProbe", "Root") is null);
        }
        finally { directory.Delete(true); }
        Contracts();
    }

    // Keep synthetic behavior checks runnable without downloading or locating the proprietary ACT oracle.
    private static void Contracts()
    {
        PluginBindingChecks.Run();
        SpeechChecks.Run();
        SoundChecks.Run();
        TextExportChecks.Run();
        SpellTimerChecks.Run();
        Eq2HostChecks.Run();
        PluginMetadataChecks.Run();
        LogFollowerChecks.Run();
        InitialBoundaryChecks.Run();
        InitialSnapshotChecks.Run();
        LogHostChecks.Run();
        RegistrationLifetimeChecks.Run();
        DebugLogChecks.Run();
        CombatPolicyChecks.Run();
        ParentSettingsChecks.Run();
        CustomTriggerChecks.Run();
        DataCorrectionChecks.Run();
        EncounterArchiveChecks.Run();
        EncounterLogChecks.Run();
        RootCombatStateChecks.Run();
        RootActionChecks.Run();
        HitRateColumnChecks.Run();
        ImportLogReaderChecks.Run();
        ImportChecks.Run();
        OriginalLogCaptureChecks.Run();
        LogBufferChecks.Run();
        CombatSnapshotChecks.Run();
        CombatQueryChecks.Run();
        CombatSnapshotEncodingChecks.Run();
        RecordedActionTranscriptChecks.Run();
        OriginalCorpusMessageChecks.Run();
        OwnerContextChecks.Run();
        HostLifetimeChecks.Run();
        DiagnosticChecks.Run();
    }
    private static void CheckRegistrationViews(FormActMain main)
    {
        var incoming = CombatantData.IncomingDamageTypeDataObjects;
        var links = CombatantData.SwingTypeToDamageTypeDataLinksIncoming;
        var columns = CombatantData.ColumnDefs;
        object Read(string name) => main.GetType().GetProperty(name, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(main)!;
        try
        {
            CombatantData.IncomingDamageTypeDataObjects = new() { ["registered"] = new("Damage", 1, System.Drawing.Color.Red) };
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming = new() { [1] = ["registered"] };
            main.ValidateLists();
            var categories = (string[])Read("IncomingCategories");
            Check(categories.SequenceEqual(new[] { "registered" }));
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming[1] = ["missing"];
            try { main.ValidateLists(); throw new Exception("Missing category accepted"); }
            catch (InvalidOperationException) { Check(ReferenceEquals(categories, Read("IncomingCategories"))); }
            CombatantData.ColumnDefs = new()
            {
                ["visible"] = new("Visible", true, "TEXT", "v", _ => "v", _ => "v", (_, _) => 0),
                ["hidden"] = new("Hidden", false, "TEXT", "h", _ => "h", _ => "h", (_, _) => 0)
            };
            main.ValidateTableSetup();
            var view = (Dictionary<Type, string[]>)Read("VisibleColumns");
            Check(view[typeof(CombatantData)].SequenceEqual(new[] { "visible" }));
            CombatantData.ColumnDefs = null!;
            try { main.ValidateTableSetup(); throw new Exception("Invalid registry accepted"); }
            catch (ArgumentNullException) { Check(ReferenceEquals(view, Read("VisibleColumns"))); }
        }
        finally
        {
            CombatantData.IncomingDamageTypeDataObjects = incoming;
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming = links;
            CombatantData.ColumnDefs = columns;
        }
    }
    private static void LinuxCompatibility()
    {
        WriterCompatibilityChecks.Run();
        ResourceCompatibilityChecks.Run("binaries/FFXIV_ACT_Plugin_SDK_3.0.3.3/SDK\\FFXIV_ACT_Plugin.Resource.dll");
        var path = "binaries/FFXIV_ACT_Plugin_SDK_3.0.3.3/SDK\\FFXIV_ACT_Plugin.Memory.dll";
        var original = File.ReadAllBytes(path);
        var before = SHA256.HashData(original);
        var rewritten = LinuxPluginCompatibility.RewriteMemory(original);
        Check(SHA256.HashData(original).AsSpan().SequenceEqual(before));
        Check(SHA256.HashData(File.ReadAllBytes(path)).AsSpan().SequenceEqual(before));
        Check(rewritten.Length == original.Length && !ReferenceEquals(original, rewritten));
        using var stream = new MemoryStream(rewritten, false);
        using var pe = new PEReader(stream);
        var metadata = pe.GetMetadataReader();
        Check(metadata.GetGuid(metadata.GetModuleDefinition().Mvid) == new Guid("89b13c66-df57-43bf-8217-c84f5c77f5f6"));
        var method = metadata.GetMethodDefinition(MetadataTokens.MethodDefinitionHandle(0x96));
        Check(metadata.GetString(method.Name) == "SetIsAdministratorFlag");
        var il = pe.GetMethodBody(method.RelativeVirtualAddress).GetILBytes()!;
        Check(Convert.ToHexString(il.AsSpan(0, 6)) == "02288E000006" && il[6] == 0x16);
        Check(il[7..26].All(value => value == 0) && Convert.ToHexString(il.AsSpan(26, 6)) == "6F820000062A");
        for (var index = 0; index < original.Length; index++)
            if (index < 9743 || index > 9762) Check(original[index] == rewritten[index]);
        void Rejected(byte[] bytes)
        {
            try { LinuxPluginCompatibility.RewriteMemory(bytes); throw new Exception("Unrecognized assembly was rewritten"); }
            catch (InvalidOperationException error) { Check(error.Message.Contains("Unsupported FFXIV memory assembly")); }
        }
        Rejected([]);
        Rejected(rewritten);
        var changed = (byte[])original.Clone();
        changed[100] ^= 1;
        Rejected(changed);
        Check(!LinuxPluginCompatibility.IsKnownPlugin(new byte[32]));
        var main = File.ReadAllBytes("binaries/FFXIV_ACT_Plugin_SDK_3.0.3.3/FFXIV_ACT_Plugin.dll");
        Check(LinuxPluginCompatibility.IsKnownPlugin(SHA256.HashData(main)) == OperatingSystem.IsLinux());
    }
}
