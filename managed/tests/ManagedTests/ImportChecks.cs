using Aftercast.Testing;
using System.Drawing;
using System.Runtime.CompilerServices;
using System.Text;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class ImportChecks
{
    private static readonly DateTime Epoch = DateTime.UnixEpoch;

    private static void Check(bool condition, [CallerArgumentExpression(nameof(condition))] string? expression = null)
    {
        if (!condition) throw new Exception("Import check failed: " + expression);
    }

    private static void Throws<T>(Action action) where T : Exception
    {
        try { action(); }
        catch (T) { return; }
        throw new Exception("Expected import failure: " + typeof(T).Name);
    }

    private sealed class Fixture : IDisposable
    {
        internal readonly DirectoryInfo Directory = System.IO.Directory.CreateTempSubdirectory("aftercast-import-");
        internal readonly HostEnvironment Host;
        internal readonly FormActMain Root;
        internal readonly ZoneData LiveZone;
        internal string Path => System.IO.Path.Combine(Directory.FullName, "import.log");
        internal Fixture(Action? pump = null)
        {
            Host = new(Directory.FullName, pump);
            Root = ActGlobals.oFormActMain!;
            CombatFixture.ConfigureCategories();
            Root.LastKnownTime = Epoch.AddDays(1);
            Root.ChangeZone("Live Zone");
            Root.LogFilePath = "live.log";
            Root.ReadThreadLock = true;
            LiveZone = Root.ActiveZone;
            Root.GetDateTimeFromLog = line => line switch
            {
                "first" => Epoch,
                "second" => Epoch.AddSeconds(2),
                _ => DateTime.MinValue
            };
        }
        internal void CheckRestored()
        {
            Check(ReferenceEquals(Root.ActiveZone, LiveZone));
            Check(Root.LogFilePath == "live.log" && Root.CurrentZone == "Live Zone");
            Check(Root.LastKnownTime == Epoch.AddDays(1) && ActGlobals.charName == "Source");
            Check(Root.LastHostileTime == DateTime.MinValue);
            Check(Root.ReadThreadLock && !Root.InCombat);
        }
        public void Dispose()
        {
            try { Host.Dispose(); }
            finally { Directory.Delete(true); }
        }
    }

    internal static void Run()
    {
        HostLifetimeChecks.RunBounded(ImportRangeChecks.Run);
        ImportBoundaryChecks.Run();
        ClipboardImportChecks.Run();
        HostLifetimeChecks.RunBounded(SuccessAndRepeated);
        HostLifetimeChecks.RunBounded(EmptyAndAdmission);
        HostLifetimeChecks.RunBounded(FailureRestoration);
        HostLifetimeChecks.RunBounded(IdleSegmentation);
        HostLifetimeChecks.RunBounded(LiveReaderResume);
        HostLifetimeChecks.RunBounded(CooperativePreparation);
        HostLifetimeChecks.RunBounded(CooperativeIdleBoundary);
        Console.WriteLine("Root import dispatch, aggregation, exclusivity and restoration passed.");
    }

    private static void CooperativeIdleBoundary()
    {
        using var context = new Aftercast.Bridge.OwnerContext(error => throw new Exception("Idle boundary dispatch failed", error));
        using var fixture = new Fixture(() => context.Pump());
        using var entered = new ManualResetEventSlim();
        using var release = new ManualResetEventSlim();
        var root = fixture.Root;
        root.GetDateTimeFromLog = line => line == "first" ? Epoch : Epoch.AddSeconds(8);
        root.AfterCombatAction += (_, info) =>
        {
            if (info.damage.Number != 100) return;
            entered.Set();
            if (!release.Wait(TimeSpan.FromSeconds(5))) throw new Exception("Idle boundary blocked owner progress");
        };
        root.OnLogLineRead += (_, info) =>
        {
            AddDamage(root, info);
            if (info.originalLogLine != "first") return;
            Check(entered.Wait(TimeSpan.FromSeconds(5)));
            // A synchronous drain pumps this release itself; a cooperative step returns before it runs.
            context.Post(_ => release.Set(), null);
        };
        File.WriteAllText(fixture.Path, "first\nsecond\n");
        fixture.Host.BeginImport(fixture.Path);
        ImportResult? result = null;
        try
        {
            do
            {
                result = fixture.Host.AdvanceImport(false);
                if (result is not null) break;
                if (fixture.Host.ImportProgress.RecordsDispatched == 0) Thread.Sleep(1);
            } while (fixture.Host.ImportProgress.RecordsDispatched == 0);
            Check(result is null && fixture.Host.ImportProgress.RecordsDispatched == 1 && !release.IsSet);
        }
        finally
        {
            release.Set(); context.Pump();
            while (result is null) { result = fixture.Host.AdvanceImport(false); if (result is null) Thread.Sleep(1); }
        }
        Check(result is { Encounters: 2, Damage: 400 });
        fixture.CheckRestored();
    }

    private static void CooperativePreparation()
    {
        using var fixture = new Fixture();
        using var entered = new ManualResetEventSlim();
        using var release = new ManualResetEventSlim();
        using var context = new Aftercast.Bridge.OwnerContext(error => throw new Exception("Preparation dispatch failed", error));
        var root = fixture.Root;
        var path = System.IO.Path.Combine(fixture.Directory.FullName, "blocked-live.log");
        File.WriteAllText(path, "");
        File.WriteAllText(fixture.Path, "first\n");
        root.ReadThreadLock = false;
        root.LogFilePath = path;
        root.OnLogLineRead += (import, _) =>
        {
            if (import) return;
            entered.Set();
            if (!release.Wait(TimeSpan.FromSeconds(5))) throw new Exception("Preparation blocked the owner");
        };
        try
        {
            root.OpenLog(false, false);
            File.AppendAllText(path, "blocked\n");
            Check(entered.Wait(TimeSpan.FromSeconds(5)));
            // Begin and cancel must remain usable while the previous live callback is still running.
            fixture.Host.BeginImport(fixture.Path);
            Check(fixture.Host.AdvanceImport(false) is null && fixture.Host.ImportProgress.LinesRead == 0);
            Throws<OperationCanceledException>(() => fixture.Host.AdvanceImport(true));
            Check(root.LogFilePath == path && !root.ReadThreadLock && !root.InCombat);
            var archive = context.RunOperation(() => fixture.Host.ArchiveOperationAsync(3, "", 0, false));
            Check(!archive.IsCompleted);
            release.Set();
            Check(SpinWait.SpinUntil(() => { context.Pump(); return archive.IsCompleted; }, TimeSpan.FromSeconds(5)));
            Check(archive.GetAwaiter().GetResult() == 0 && fixture.Host.Diagnostics.Item1 == 0);
        }
        finally { release.Set(); fixture.Host.StopLogReading(); }
    }

    private static void AddDamage(FormActMain root, LogLineEventArgs info)
    {
        if (info.originalLogLine is not ("first" or "second")) return;
        root.SetEncounter(info.detectedTime, "Source", "Target");
        root.AddCombatAction(new(1, false, "None", new Dnum(info.originalLogLine == "first" ? 100 : 300),
            info.detectedTime, root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
    }

    private static void SuccessAndRepeated()
    {
        using var fixture = new Fixture();
        var root = fixture.Root;
        File.WriteAllText(fixture.Path, "first\n\nsecond\nno-time\n", new UTF8Encoding(false));
        var notifications = new List<(bool Import, string Path)>();
        root.LogFileChanged += (import, path) =>
        {
            // Restoration subscribers must see the complete live context, e.g. the original zone and clock.
            if (!import) fixture.CheckRestored();
            notifications.Add((import, path));
        };
        var before = 0;
        root.BeforeLogLineRead += (import, info) =>
        {
            Check(import && root.ReadThreadLock && root.LogFilePath == fixture.Path);
            Check(ReferenceEquals(root.ActiveZone, root.ZoneList[0]));
            if (info.originalLogLine == "first")
            {
                Check(info.detectedZone == "Import Zone");
                info.logLine += " amended";
                // Reentrant lifecycle changes and foreign producers cannot take ownership of this parser graph.
                Throws<InvalidOperationException>(() => fixture.Host.ImportLog(fixture.Path));
                Throws<InvalidOperationException>(() => root.OpenLog(false, false));
                Throws<InvalidOperationException>(() => fixture.Host.StopProcessing());
                Exception? foreign = null;
                var worker = new Thread(() =>
                {
                    try { root.SetEncounter(Epoch, "Foreign", "Target"); }
                    catch (Exception error) { foreign = error; }
                })
                { IsBackground = true };
                worker.Start();
                Check(worker.Join(TimeSpan.FromSeconds(5)) && foreign is InvalidOperationException);
            }
            if (info.originalLogLine == "no-time")
            {
                Check(info.detectedTime == DateTime.MinValue && root.LastKnownTime == Epoch.AddSeconds(2));
                ActGlobals.charName = "Imported character";
            }
            before++;
        };
        root.OnLogLineRead += (import, info) =>
        {
            Check(import && (info.originalLogLine != "first" || info.logLine == "first amended"));
            AddDamage(root, info);
        };
        var sorter = root.GlobalTimeSorter;
        var report = fixture.Host.ImportLog(fixture.Path);
        Check(report == new ImportResult(4, 3, 1, 2, 400, 0));
        Check(before == 3 && root.GlobalTimeSorter == sorter + 3);
        Check(notifications.SequenceEqual(new[] { (true, fixture.Path), (false, "live.log") }));
        fixture.CheckRestored();
        var firstEncounter = root.ZoneList[0].Items[0];
        Check(!firstEncounter.Active && firstEncounter.Damage == 400 && ReferenceEquals(firstEncounter.Parent, root.ZoneList[0]));
        Check(fixture.Host.ImportLog(fixture.Path) == report && root.ZoneList[0].Items.Count == 2);
        fixture.CheckRestored();
        Check(fixture.Host.Diagnostics.Item1 == 0);
    }

    private static void EmptyAndAdmission()
    {
        using var fixture = new Fixture();
        var root = fixture.Root;
        File.WriteAllText(fixture.Path, "\n\n", new UTF8Encoding(false));
        var changed = 0;
        root.LogFileChanged += (_, _) => changed++;
        root.OnLogLineRead += (_, _) => throw new Exception("Empty lines must not be dispatched");
        var sorter = root.GlobalTimeSorter;
        Check(fixture.Host.ImportLog(fixture.Path) == new ImportResult(2, 0, 0, 0, 0, 0));
        Check(changed == 2 && root.GlobalTimeSorter == sorter);
        fixture.CheckRestored();
        Throws<ArgumentException>(() => fixture.Host.ImportLog("relative.log"));
        root.SetEncounter(Epoch, "Source", "Target");
        // Reject before opening a missing source or notifying the parser when live combat is active.
        Throws<InvalidOperationException>(() => fixture.Host.ImportLog(fixture.Path + ".missing"));
        Check(changed == 2 && root.InCombat);
        root.EndCombat(false);
    }

    private static void FailureRestoration()
    {
        using var fixture = new Fixture();
        var root = fixture.Root;
        root.OnLogLineRead += (_, info) => AddDamage(root, info);
        // A malformed source retains its already-aggregated encounter and restores every live field.
        File.WriteAllBytes(fixture.Path, Encoding.UTF8.GetBytes("first\n").Concat(new byte[] { 0xff, 0x0a }).ToArray());
        Throws<InvalidDataException>(() => fixture.Host.ImportLog(fixture.Path));
        fixture.CheckRestored();
        Check(root.ZoneList[0].Items.Count == 1 && root.ZoneList[0].Items[0].Damage == 100);
        File.WriteAllText(fixture.Path, "first\nsecond\n", new UTF8Encoding(false));
        LogLineEventDelegate failure = (_, info) =>
        {
            if (info.originalLogLine == "first") throw new InvalidOperationException("Fixture import subscriber failure");
        };
        root.BeforeLogLineRead += failure;
        // Isolation still dispatches later hooks/records, but a failed parser callback cannot produce a success report.
        Throws<InvalidDataException>(() => fixture.Host.ImportLog(fixture.Path));
        root.BeforeLogLineRead -= failure;
        fixture.CheckRestored();
        Check(root.ZoneList[0].Items[^1].Damage == 400 && fixture.Host.Diagnostics.Item1 == 1);
        Check(fixture.Host.ImportLog(fixture.Path).Damage == 400);
        fixture.CheckRestored();
    }

    private static void IdleSegmentation()
    {
        using var fixture = new Fixture();
        var root = fixture.Root;
        root.GetDateTimeFromLog = line => line == "first" ? Epoch : Epoch.AddSeconds(8);
        root.OnLogLineRead += (_, info) => AddDamage(root, info);
        File.WriteAllText(fixture.Path, "first\nsecond\n", new UTF8Encoding(false));
        // The idle check precedes the second raw hook, so it must create a separate individual encounter.
        Check(fixture.Host.ImportLog(fixture.Path) == new ImportResult(2, 2, 2, 4, 400, 0));
        Check(root.ZoneList.Count == 2 && root.ZoneList[0].Items.All(encounter => !encounter.Active));
        fixture.CheckRestored();
    }

    private static void LiveReaderResume()
    {
        using var fixture = new Fixture();
        var root = fixture.Root;
        var livePath = System.IO.Path.Combine(fixture.Directory.FullName, "live.log");
        File.WriteAllText(livePath, "existing\n", new UTF8Encoding(false));
        root.LogFilePath = livePath;
        root.ReadThreadLock = false;
        root.OpenLog(false, false);
        File.WriteAllText(fixture.Path, "first\n", new UTF8Encoding(false));
        using var liveDelivered = new ManualResetEventSlim();
        root.OnLogLineRead += (import, info) =>
        {
            if (import)
            {
                File.AppendAllText(livePath, "after-import\n", new UTF8Encoding(false));
                Check(!liveDelivered.IsSet);
            }
            else if (info.logLine == "after-import") liveDelivered.Set();
        };
        Check(fixture.Host.ImportLog(fixture.Path).RecordsDispatched == 1);
        Check(liveDelivered.Wait(TimeSpan.FromSeconds(5)));
        Check(root.LogFilePath == livePath && !root.ReadThreadLock && ReferenceEquals(root.ActiveZone, fixture.LiveZone));
        fixture.Host.StopLogReading();
    }
}
