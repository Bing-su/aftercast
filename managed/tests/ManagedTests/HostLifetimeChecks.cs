using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Bridge;

internal static class HostLifetimeChecks
{
    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception("Host lifetime check failed: " + message);
    }

    internal static void Run()
    {
        RunBounded(() => CheckLifetime(stop: true));
        RunBounded(() => CheckLifetime(stop: false));
        RunBounded(RejectDispatchedLifecycleWaits);
        RunBounded(StopDuringReplacement);
        Console.WriteLine("Pumped reader shutdown and quiescent plugin teardown checks passed.");
    }

    private static void StopDuringReplacement()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-log-replacement-");
        try
        {
            using var context = new OwnerContext(_ => throw new Exception("Unexpected posted failure"));
            using var control = context.Run(() => UiChecks.CreateControl());
            using var environment = new HostEnvironment(directory.FullName, () => context.Pump(), () => context.IsDispatching);
            var root = ActGlobals.oFormActMain!;
            var first = Path.Combine(directory.FullName, "first.log");
            var second = Path.Combine(directory.FullName, "second.log");
            File.WriteAllText(first, "");
            File.WriteAllText(second, "");
            using var entered = new ManualResetEventSlim();
            using var completed = new ManualResetEventSlim();
            root.GetDateTimeFromLog = _ => DateTime.UnixEpoch;
            root.BeforeLogLineRead += (_, _) =>
            {
                entered.Set();
                control.Invoke((Action)(() => control.Text = "drained"));
                completed.Set();
            };
            root.LogFilePath = first;
            root.OpenLog(false, false);
            File.AppendAllText(first, "pending\n");
            Check(entered.Wait(TimeSpan.FromSeconds(5)), "replacement reader did not enter callback");
            Exception? replacementFailure = null;
            var replacement = new Thread(() =>
            {
                try { root.LogFilePath = second; root.OpenLog(false, false); }
                catch (Exception error) { replacementFailure = error; }
            })
            { IsBackground = true };
            replacement.Start();
            var field = root.GetType().GetField("follower", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!;
            Check(SpinWait.SpinUntil(() => field.GetValue(root) is null, TimeSpan.FromSeconds(5)), "replacement did not retire old reader");
            // The current pointer is empty, but shutdown must still find and drain the retired reader.
            environment.StopLogReading();
            Check(replacement.Join(TimeSpan.FromSeconds(5)) && replacementFailure is null && completed.IsSet,
                "shutdown stranded a retired reader or its replacement");
            Check(control.Text == "drained" && environment.Diagnostics.Item1 == 0, "retired callback failed during shutdown");
        }
        finally { directory.Delete(true); }
    }

    internal static void RunBounded(Action action)
    {
        using var done = new ManualResetEventSlim();
        Exception? failure = null;
        var owner = new Thread(() =>
        {
            try { action(); }
            catch (Exception error) { failure = error; }
            finally { done.Set(); }
        })
        { IsBackground = true };
        if (OperatingSystem.IsWindows()) owner.SetApartmentState(ApartmentState.STA);
        owner.Start();
        // Keep a broken shutdown join from hanging the entire test process.
        Check(done.Wait(TimeSpan.FromSeconds(15)), "owner-thread shutdown or suspension deadlocked");
        Check(owner.Join(TimeSpan.FromSeconds(1)), "owner thread did not exit");
        if (failure is not null) System.Runtime.ExceptionServices.ExceptionDispatchInfo.Capture(failure).Throw();
    }

    private static void CheckLifetime(bool stop)
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-lifetime-");
        try
        {
            using var context = new OwnerContext(_ => throw new Exception("Unexpected posted callback failure"));
            using var control = context.Run(() => UiChecks.CreateControl());
            using var environment = new HostEnvironment(directory.FullName, () => context.Pump(), () => context.IsDispatching);
            var root = ActGlobals.oFormActMain!;
            var path = Path.Combine(directory.FullName, "Network.log");
            File.WriteAllText(path, "");
            root.LogFilePath = path;
            root.GetDateTimeFromLog = _ => DateTime.UnixEpoch;
            using var entered = new ManualResetEventSlim();
            using var callbackDone = new ManualResetEventSlim();
            using var futureDone = new ManualResetEventSlim();
            var callbackOnOwner = false;
            var actionRunning = 0;
            var futureDuringAction = false;
            var owner = Environment.CurrentManagedThreadId;
            LogLineEventDelegate oldHandler = (_, _) =>
            {
                entered.Set();
                control.Invoke((Action)(() => callbackOnOwner = Environment.CurrentManagedThreadId == owner));
                callbackDone.Set();
            };
            root.BeforeLogLineRead += oldHandler;
            root.OpenLog(false, false);
            File.AppendAllText(path, "first\n");
            Check(entered.Wait(TimeSpan.FromSeconds(5)), "reader callback did not enter");
            // The owner intentionally does not pump here: stop/suspension must drain the queued Invoke itself.
            if (stop)
            {
                environment.StopLogReading();
                Check(callbackDone.IsSet && callbackOnOwner, "stop returned before pumped callback completed");
            }
            else
            {
                environment.RunWithLogReadingSuspended(() =>
                {
                    Check(callbackDone.IsSet && callbackOnOwner, "teardown began before existing callback finished");
                    root.BeforeLogLineRead -= oldHandler;
                    root.BeforeLogLineRead += (_, _) =>
                    {
                        futureDuringAction = Volatile.Read(ref actionRunning) != 0;
                        futureDone.Set();
                    };
                    Volatile.Write(ref actionRunning, 1);
                    File.AppendAllText(path, "after teardown\n");
                    Check(!futureDone.Wait(TimeSpan.FromMilliseconds(100)), "reader dispatched while teardown was suspended");
                    Volatile.Write(ref actionRunning, 0);
                });
                Check(futureDone.Wait(TimeSpan.FromSeconds(5)) && !futureDuringAction, "reader did not resume after quiescent teardown");
            }
            environment.Dispose();
            Check(ActGlobals.oFormActMain is null && environment.Diagnostics.Item1 == 0, "shutdown leaked globals or reported callback failures");
        }
        finally { directory.Delete(true); }
    }

    private static void RejectDispatchedLifecycleWaits()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-reentrant-");
        try
        {
            using var context = new OwnerContext(_ => throw new Exception("Unexpected posted callback failure"));
            using var control = context.Run(() => UiChecks.CreateControl());
            using var environment = new HostEnvironment(directory.FullName, () => context.Pump(), () => context.IsDispatching);
            var root = ActGlobals.oFormActMain!;
            var path = Path.Combine(directory.FullName, "Network.log");
            File.WriteAllText(path, "");
            root.LogFilePath = path;
            root.GetDateTimeFromLog = _ => DateTime.UnixEpoch;
            using var callbackDone = new ManualResetEventSlim();
            using var secondDone = new ManualResetEventSlim();
            var rejected = 0;
            var teardownRan = false;
            object? followerBefore = null;
            object? ReadFollower() => root.GetType().GetField("follower", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!.GetValue(root);
            void Reject(Action action)
            {
                try { action(); throw new Exception("A dispatched callback entered a lifecycle wait"); }
                catch (InvalidOperationException) { rejected++; }
            }
            root.BeforeLogLineRead += (_, info) =>
            {
                if (info.logLine == "second") { secondDone.Set(); return; }
                control.Invoke((Action)(() =>
                {
                    // The reader is waiting for this owner callback; lifecycle joins must reject before mutation.
                    Check(context.IsDispatching, "owner callback was not marked as dispatching");
                    Reject(() => root.OpenLog(false, false));
                    Reject(environment.StopLogReading);
                    Reject(() => environment.RunWithLogReadingSuspended(() => teardownRan = true));
                    Reject(root.Dispose);
                    Check(ReferenceEquals(ReadFollower(), followerBefore) && !root.IsDisposed && ReferenceEquals(root, ActGlobals.oFormActMain), "rejected lifecycle action changed follower or root state");
                }));
                callbackDone.Set();
            };
            root.OpenLog(false, false);
            followerBefore = ReadFollower();
            File.AppendAllText(path, "first\n");
            Check(SpinWait.SpinUntil(() => { context.Pump(); return callbackDone.IsSet; }, TimeSpan.FromSeconds(5)), "dispatched lifecycle rejection stranded the reader");
            Check(rejected == 4 && !teardownRan, "lifecycle action was not rejected before teardown");
            File.AppendAllText(path, "second\n");
            Check(secondDone.Wait(TimeSpan.FromSeconds(5)), "rejected lifecycle action stopped later records");
            environment.StopLogReading();
            environment.Dispose();
            Check(ActGlobals.oFormActMain is null && environment.Diagnostics.Item1 == 0, "ordinary shutdown failed after dispatched rejection");
        }
        finally { directory.Delete(true); }
    }
}
