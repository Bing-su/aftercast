using Aftercast.Testing;
using System.Drawing;
using System.Reflection;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Bridge;

internal static class RootActionChecks
{
    private static readonly DateTime Epoch = DateTime.UnixEpoch;
    private static void Check(bool condition, [System.Runtime.CompilerServices.CallerArgumentExpression(nameof(condition))] string? expression = null)
    {
        if (!condition) throw new Exception("Root action check failed: " + expression);
    }

    private static void Throws<T>(Action action) where T : Exception
    {
        try { action(); }
        catch (T) { return; }
        throw new Exception($"Root action operation did not throw {typeof(T).Name}");
    }

    private sealed class Fixture : IDisposable
    {
        private readonly DirectoryInfo directory = Directory.CreateTempSubdirectory("aftercast-combat-");
        internal readonly HostEnvironment Environment;
        internal readonly FormActMain Root;
        internal string LogPath => Path.Combine(directory.FullName, "Network.log");
        internal Fixture(OwnerContext? context = null)
        {
            Environment = new(directory.FullName, context is null ? null : () => context.Pump(),
                context is null ? null : () => context.IsDispatching);
            Root = ActGlobals.oFormActMain!;
            CombatFixture.ConfigureCategories();
            Root.LastKnownTime = Epoch;
            Root.ChangeZone("Fixture Zone");
        }
        public void Dispose()
        {
            try { Environment.Dispose(); }
            finally { directory.Delete(true); }
        }
    }

    private static MasterSwing Swing(long amount, int second, int sorter) =>
        new(1, false, " \t ", new Dnum(amount), Epoch.AddSeconds(second), sorter,
            " Strike ", " Source ", "Physical", " Target ");

    internal static void Run()
    {
        HostLifetimeChecks.RunBounded(Pipeline);
        HostLifetimeChecks.RunBounded(IdleBoundary);
        HostLifetimeChecks.RunBounded(IdleOptions);
        HostLifetimeChecks.RunBounded(IdleTimer);
        HostLifetimeChecks.RunBounded(IdleTimerDuringImport);
        HostLifetimeChecks.RunBounded(EndCommand);
        HostLifetimeChecks.RunBounded(AdmittedProducerDuringEnd);
        HostLifetimeChecks.RunBounded(PumpedShutdown);
        HostLifetimeChecks.RunBounded(() => ShutdownDuringTransition(starting: true));
        HostLifetimeChecks.RunBounded(() => ShutdownDuringTransition(starting: false));
        HostLifetimeChecks.RunBounded(QuiescentTeardown);
        HostLifetimeChecks.RunBounded(RejectCrossPipelineJoin);
        HostLifetimeChecks.RunBounded(BacklogContinues);
        Console.WriteLine("Root combat hooks, admitted-action draining, idle boundaries, pumped shutdown and backlog continuation passed.");
    }

    private static void Pipeline()
    {
        using var fixture = new Fixture();
        var root = fixture.Root;
        Throws<InvalidOperationException>(() => root.AddCombatAction(Swing(1, 0, 0)));
        var starts = 0;
        root.OnCombatStart += (_, info) =>
        {
            Check(!root.InCombat && root.LastHostileTime == DateTime.MinValue);
            Check(ReferenceEquals(info.encounter, root.ActiveZone.ActiveEncounter));
            Throws<InvalidOperationException>(() => root.SetEncounter(Epoch, "Source", "Target"));
            starts++;
        };
        Check(root.SetEncounter(Epoch, "Source", "Target") && starts == 1 && root.InCombat);
        var owner = System.Environment.CurrentManagedThreadId;
        var beforeCalls = 0;
        var afterCalls = 0;
        var action = Swing(20, 0, 1);
        var originalTags = action.Tags;
        root.BeforeCombatAction += (_, info) =>
        {
            Check(System.Environment.CurrentManagedThreadId == owner);
            Check(info.special == "None" && info.attacker == "Source" && info.theAttackType == "Strike");
            beforeCalls++;
            if (info.timeSorter == 99) { info.cancelAction = true; return; }
            if (info.timeSorter == 1)
            {
                info.damage = new Dnum(100);
                info.attacker = new string("Source".ToCharArray());
                info.tags["shared"] = 1;
                info.tags = new() { ["replaced"] = true };
            }
            if (info.timeSorter == 2) throw new InvalidOperationException("Fixture Before failure");
        };
        root.BeforeCombatAction += (_, info) => Check(info.timeSorter != 99);
        root.AfterCombatAction += (_, info) =>
        {
            Check(System.Environment.CurrentManagedThreadId != owner);
            Check(info.combatAction.ParentEncounter!.GetCombatant("Source")!.Damage >= info.damage.Number);
            if (info.timeSorter == 1) throw new InvalidOperationException("Fixture After failure");
        };
        root.AfterCombatAction += (_, _) => Interlocked.Increment(ref afterCalls);
        root.AddCombatAction(action);
        root.AddCombatAction(Swing(300, 2, 2));
        root.AddCombatAction(Swing(999, 3, 99));
        var ends = 0;
        root.OnCombatEnd += (_, info) =>
        {
            Check(!root.InCombat && afterCalls == 2 && info.encounter!.Damage == 400);
            Check(info.encounter!.Duration == TimeSpan.FromSeconds(2) && info.encounter.Title == "Target");
            ends++;
        };
        root.EndCombat(true);
        Check(ends == 1 && beforeCalls == 3 && afterCalls == 2);
        Check(ReferenceEquals(action.Attacker, string.Intern("Source")) && action.Damage.Number == 100);
        Check(ReferenceEquals(action.Tags, originalTags) && action.Tags.ContainsKey("shared") && !action.Tags.ContainsKey("replaced"));
        Check(fixture.Environment.Diagnostics.Item1 == 2);
        Check(!root.InCombat);
        root.ActCommands("ordinary echo text");
    }

    private static void EndCommand()
    {
        using var fixture = new Fixture();
        var root = fixture.Root;
        root.ActCommands("end");
        Check(fixture.Environment.ReadCombatSnapshot(0) is null);
        root.SetEncounter(Epoch, "Source", "Target");
        root.AddCombatAction(Swing(100, 0, 1));
        root.AddCombatAction(Swing(300, 2, 2));
        root.ActCommands("add Player");
        Check(root.SelectiveListGetSelected("player") && root.InCombat);
        root.ActCommands("remove PLAYER");
        Check(!root.SelectiveListGetSelected("Player") && root.InCombat);
        root.ActCommands("add Player");
        root.ActCommands("reset players");
        Check(!root.SelectiveListGetSelected("Player") && root.InCombat);
        foreach (var unsupported in new[] { "macro all", "macro last",
            "macro: test", "clip all", "clip last", "clip last-1", "clear html", "clear", "timer test", null })
        {
            Throws<NotSupportedException>(() => root.ActCommands(unsupported!));
            Check(root.InCombat);
        }
        foreach (var text in new[] { "END", " end", "end ", "", "ordinary echo text", "Hojoring>[EX] Beacon X=21.50",
            "Clear", "timer", "add", "macro:fixture" })
        {
            root.ActCommands(text);
            Check(root.InCombat && fixture.Environment.ReadCombatSnapshot(0) is null);
        }
        root.ActCommands("end");
        var result = fixture.Environment.ReadCombatSnapshot(0)!;
        Check(!root.InCombat && result.Damage == 400 && result.DurationTicks == TimeSpan.FromSeconds(2).Ticks);
        root.ActCommands("end");
        Check(fixture.Environment.ReadCombatSnapshot(result.Revision) is null);
    }

    private static void IdleBoundary()
    {
        using var fixture = new Fixture();
        var root = fixture.Root;
        root.SetEncounter(Epoch, "Source", "Target");
        root.AddCombatAction(Swing(100, 0, 1));
        fixture.Environment.RunWithLogReadingSuspended(() => { });
        var checkIdle = typeof(FormActMain).GetMethod("CheckIdleEndCombat", BindingFlags.Instance | BindingFlags.NonPublic)!;
        // Parsed time drives termination; an exact six-second gap must leave combat active.
        root.LastKnownTime = Epoch.AddSeconds(6);
        checkIdle.Invoke(root, null);
        Check(root.InCombat);
        root.LastKnownTime = Epoch.AddSeconds(6).AddTicks(1);
        checkIdle.Invoke(root, null);
        Check(!root.InCombat && root.ActiveZone.ActiveEncounter!.Damage == 100);
    }

    private static void IdleOptions()
    {
        using var fixture = new Fixture();
        var root = fixture.Root;
        var check = typeof(FormActMain).GetMethod("CheckIdleEndCombat", BindingFlags.Instance | BindingFlags.NonPublic)!;
        fixture.Environment.ConfigureIdleEnd(false, 2);
        root.SetEncounter(Epoch, "Source", "Target"); root.AddCombatAction(Swing(100, 0, 1));
        fixture.Environment.RunWithLogReadingSuspended(() => { });
        root.LastKnownTime = Epoch.AddSeconds(100); check.Invoke(root, null); Check(root.InCombat);
        fixture.Environment.ConfigureIdleEnd(true, 30);
        Throws<ArgumentOutOfRangeException>(() => fixture.Environment.ConfigureIdleEnd(true, 1));
        Throws<ArgumentOutOfRangeException>(() => fixture.Environment.ConfigureIdleEnd(true, 601));
        root.LastKnownTime = Epoch.AddSeconds(30); check.Invoke(root, null); Check(root.InCombat);
        root.LastKnownTime = Epoch.AddSeconds(30).AddTicks(1); check.Invoke(root, null); Check(!root.InCombat);
    }

    private static void IdleTimer()
    {
        using var fixture = new Fixture();
        var root = fixture.Root;
        var tick = typeof(FormActMain).GetMethod("AdvanceIdleTimer", BindingFlags.Instance | BindingFlags.NonPublic)!;
        long time = 0, second = System.Diagnostics.Stopwatch.Frequency;
        void Advance(long seconds = 1) { time += second * seconds; tick.Invoke(root, [time]); }
        tick.Invoke(root, [time]);
        fixture.Environment.ConfigureIdleEnd(true, 2);
        root.SetEncounter(Epoch, "Source", "Target"); root.AddCombatAction(Swing(100, 0, 1));
        Advance(20); Check(root.InCombat); // delayed callbacks coalesce
        for (int i = 0; i < 3; i++) Advance();
        Check(root.InCombat); // four ticks, threshold is strictly greater than 2 + 2
        root.SetEncounter(Epoch.AddSeconds(1), "Source", "Target"); // resets elapsed counter
        Advance(); Check(root.InCombat);
        root.ReadThreadLock = true;
        for (int i = 0; i < 8; i++) Advance();
        Check(root.InCombat);
        root.ReadThreadLock = false;
        fixture.Environment.ConfigureIdleEnd(true, 2, false);
        for (int i = 0; i < 8; i++) Advance();
        Check(root.InCombat);
        fixture.Environment.ConfigureIdleEnd(false, 2, true);
        for (int i = 0; i < 8; i++) Advance();
        Check(root.InCombat);
        fixture.Environment.ConfigureIdleEnd(true, 2, true);
        for (int i = 0; i < 4; i++) Advance();
        Check(root.InCombat);
        var oldGeneration = (long)typeof(FormActMain).GetField("idleGeneration", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(root)!;
        Advance(); Check(!root.InCombat);
        root.SetEncounter(Epoch.AddSeconds(2), "Source", "Target");
        var end = typeof(FormActMain).GetMethod("EndCombatCore", BindingFlags.Instance | BindingFlags.NonPublic)!;
        end.Invoke(root, [true, oldGeneration]);
        Check(root.InCombat); // a late decision from the previous encounter is stale
        Advance(); Check(root.InCombat);
        fixture.Environment.Dispose();
        for (int i = 0; i < 8; i++) Advance(); // retired host cannot schedule another transition
    }

    private static void IdleTimerDuringImport()
    {
        using var fixture = new Fixture();
        var root = fixture.Root;
        var tick = typeof(FormActMain).GetMethod("AdvanceIdleTimer", BindingFlags.Instance | BindingFlags.NonPublic)!;
        var second = System.Diagnostics.Stopwatch.Frequency;
        bool observed = false;
        root.TimeStampLen = 0;
        fixture.Environment.ConfigureIdleEnd(true, 2);
        root.OnLogLineRead += (import, _) =>
        {
            if (!import) return;
            root.SetEncounter(Epoch, "Source", "Target");
            root.AddCombatAction(Swing(1, 0, 1));
            for (long n = 0; n < 12; n++) tick.Invoke(root, [n * second]);
            Check(root.InCombat);
            observed = true;
        };
        File.WriteAllText(fixture.LogPath, "synthetic\n");
        fixture.Environment.ImportLog(fixture.LogPath);
        Check(observed);
    }

    private static void AdmittedProducerDuringEnd()
    {
        using var context = new OwnerContext(_ => throw new Exception("Unexpected posted failure"));
        using var fixture = new Fixture(context);
        var root = fixture.Root;
        root.SetEncounter(Epoch, "Source", "Target");
        using var entered = new ManualResetEventSlim();
        using var release = new ManualResetEventSlim();
        root.BeforeCombatAction += (_, _) => { entered.Set(); Check(release.Wait(TimeSpan.FromSeconds(5))); };
        Exception? failure = null;
        var producer = new Thread(() =>
        {
            try { root.AddCombatAction(Swing(100, 0, 1)); }
            catch (Exception error) { failure = error; }
        })
        { IsBackground = true };
        producer.Start();
        Check(entered.Wait(TimeSpan.FromSeconds(5)));
        context.Post(_ =>
        {
            Check(!root.InCombat);
            Throws<InvalidOperationException>(() => root.AddCombatAction(Swing(200, 0, 2)));
            release.Set();
        }, null);
        root.EndCombat(true);
        Check(producer.Join(TimeSpan.FromSeconds(5)) && failure is null);
        Check(root.ActiveZone.ActiveEncounter!.Damage == 100 && fixture.Environment.Diagnostics.Item1 == 0);
    }

    private static void PumpedShutdown()
    {
        using var context = new OwnerContext(_ => throw new Exception("Unexpected posted failure"));
        using var control = context.Run(() => UiChecks.CreateControl());
        using var fixture = new Fixture(context);
        var root = fixture.Root;
        root.SetEncounter(Epoch, "Source", "Target");
        using var entered = new ManualResetEventSlim();
        var completed = false;
        root.AfterCombatAction += (_, _) =>
        {
            entered.Set();
            control.Invoke((Action)(() =>
            {
                Throws<InvalidOperationException>(() => fixture.Environment.StopProcessing());
                Throws<InvalidOperationException>(() => root.EndCombat(true));
                control.Text = "drained";
            }));
            completed = true;
        };
        root.AddCombatAction(Swing(100, 0, 1));
        Check(entered.Wait(TimeSpan.FromSeconds(5)));
        fixture.Environment.StopProcessing();
        Check(completed && control.Text == "drained" && !root.InCombat);
        Check(root.ActiveZone.ActiveEncounter!.Damage == 100 && fixture.Environment.Diagnostics.Item1 == 0);
        Throws<ObjectDisposedException>(() => root.SetEncounter(Epoch, "Source", "Target"));
        // Public ACT state setters do not authorize callbacks to resume after the host stopped.
        root.InCombat = true;
        Throws<ObjectDisposedException>(() => root.EndCombat(true));
        root.InCombat = false;
    }

    private static void QuiescentTeardown()
    {
        using var fixture = new Fixture();
        var root = fixture.Root;
        root.SetEncounter(Epoch, "Source", "Target");
        var before = 0;
        root.BeforeCombatAction += (_, _) => Interlocked.Increment(ref before);
        root.AddCombatAction(Swing(100, 0, 1));
        var workerRejected = false;
        fixture.Environment.RunWithLogReadingSuspended(() =>
        {
            Check(root.ActiveZone.ActiveEncounter!.Damage == 100 && before == 1);
            var producer = new Thread(() =>
            {
                try { root.AddCombatAction(Swing(999, 0, 2)); }
                catch (InvalidOperationException) { workerRejected = true; }
            })
            { IsBackground = true };
            producer.Start();
            Check(producer.Join(TimeSpan.FromSeconds(5)) && workerRejected);
            Check(before == 1);
            Throws<InvalidOperationException>(() => root.SetEncounter(Epoch, "Source", "Target"));
            Throws<InvalidOperationException>(() => root.ChangeZone("During teardown"));
            Throws<InvalidOperationException>(() => fixture.Environment.StopProcessing());
            // The teardown thread may close its already-drained encounter without generating new actions.
            root.EndCombat(true);
        });
        Check(root.SetEncounter(Epoch.AddSeconds(2), "Source", "Target"));
        Throws<InvalidOperationException>(() => fixture.Environment.RunWithLogReadingSuspended(() =>
            throw new InvalidOperationException("Fixture teardown failure")));
        root.AddCombatAction(Swing(50, 2, 3));
        root.EndCombat(true);
        Check(before == 2 && root.ActiveZone.ActiveEncounter!.Damage == 50);
    }

    private static void RejectCrossPipelineJoin()
    {
        using var fixture = new Fixture();
        var root = fixture.Root;
        File.WriteAllText(fixture.LogPath, "");
        root.LogFilePath = fixture.LogPath;
        root.GetDateTimeFromLog = _ => Epoch;
        root.OpenLog(false, false);
        root.SetEncounter(Epoch, "Source", "Target");
        using var readerEntered = new ManualResetEventSlim();
        using var readerDone = new ManualResetEventSlim();
        using var afterEntered = new ManualResetEventSlim();
        var rejected = false;
        root.BeforeLogLineRead += (_, _) =>
        {
            readerEntered.Set();
            root.EndCombat(true);
            readerDone.Set();
        };
        root.AfterCombatAction += (_, _) =>
        {
            afterEntered.Set();
            Check(readerEntered.Wait(TimeSpan.FromSeconds(5)));
            // The reader is draining this action, so joining it here would create a cycle without a UI callback.
            Throws<InvalidOperationException>(() => root.OpenLog(false, false));
            Throws<InvalidOperationException>(() => fixture.Environment.StopLogReading());
            rejected = true;
        };
        root.AddCombatAction(Swing(100, 0, 1));
        Check(afterEntered.Wait(TimeSpan.FromSeconds(5)));
        File.AppendAllText(fixture.LogPath, "end combat\n");
        Check(readerDone.Wait(TimeSpan.FromSeconds(5)) && rejected && !root.InCombat);
        Check(root.ActiveZone.ActiveEncounter!.Damage == 100 && fixture.Environment.Diagnostics.Item1 == 0);
    }

    private static void ShutdownDuringTransition(bool starting)
    {
        using var context = new OwnerContext(_ => throw new Exception("Unexpected posted failure"));
        using var control = context.Run(() => UiChecks.CreateControl());
        using var fixture = new Fixture(context);
        var root = fixture.Root;
        if (!starting)
        {
            root.SetEncounter(Epoch, "Source", "Target");
            root.AddCombatAction(Swing(100, 0, 1));
            fixture.Environment.RunWithLogReadingSuspended(() => { });
        }
        using var entered = new ManualResetEventSlim();
        var completed = false;
        CombatToggleEventDelegate callback = (_, _) =>
        {
            entered.Set();
            control.Invoke((Action)(() =>
            {
                Throws<InvalidOperationException>(() => fixture.Environment.StopProcessing());
                control.Text = "transition drained";
            }));
            completed = true;
        };
        if (starting) root.OnCombatStart += callback;
        else root.OnCombatEnd += callback;
        Exception? failure = null;
        var producer = new Thread(() =>
        {
            try
            {
                if (starting) root.SetEncounter(Epoch, "Source", "Target");
                else root.EndCombat(true);
            }
            catch (Exception error) { failure = error; }
        })
        { IsBackground = true };
        producer.Start();
        Check(entered.Wait(TimeSpan.FromSeconds(5)));
        // A transition admitted before shutdown must finish before its events or globals are released.
        fixture.Environment.StopProcessing();
        Check(producer.Join(TimeSpan.FromSeconds(5)) && failure is null && completed);
        Check(control.Text == "transition drained" && !root.InCombat && fixture.Environment.Diagnostics.Item1 == 0);
    }

    private static void BacklogContinues()
    {
        using var fixture = new Fixture();
        var root = fixture.Root;
        root.SetEncounter(Epoch, "Source", "Target");
        using var entered = new ManualResetEventSlim();
        using var release = new ManualResetEventSlim();
        var after = 0;
        root.AfterCombatAction += (_, _) =>
        {
            if (Interlocked.Increment(ref after) == 1) { entered.Set(); Check(release.Wait(TimeSpan.FromSeconds(10))); }
        };
        root.AddCombatAction(Swing(1, 0, 1));
        Check(entered.Wait(TimeSpan.FromSeconds(5)));
        try
        {
            for (var index = 0; index < 4096; index++) root.AddCombatAction(Swing(1, 0, index + 2));
            Check(root.InCombat && fixture.Environment.Diagnostics.Item1 == 1);
            Check(fixture.Environment.Errors.Item1 == 0);
        }
        finally { release.Set(); }
        root.EndCombat(true);
        Check(after == 4097 && root.ActiveZone.ActiveEncounter!.Damage == 4097);
        Check(fixture.Environment.Diagnostics.Item1 == 1);
    }
}
