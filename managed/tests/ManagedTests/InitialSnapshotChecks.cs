using System.Collections.Concurrent;
using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Text;
using Advanced_Combat_Tracker;
using Bxivact.ActCompat;

internal static class InitialSnapshotChecks
{
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("bxivact-initial-identity-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            root.ZoneChangeRegex = new("^zone:(?<zone>.+)$");
            var path = Path.Combine(directory.FullName, "zone.log");
            File.WriteAllText(path, "zone:original-long-zone\n", new UTF8Encoding(true));
            var type = typeof(FormActMain).Assembly.GetType("Advanced_Combat_Tracker.LogFollower", true)!;
            var lines = new ConcurrentQueue<string>();
            var errors = new ConcurrentQueue<Exception>();
            // Explicit initial mode blocks the worker until the root has finished reading this exact open image.
            using var reader = (IDisposable)type.GetConstructor(BindingFlags.Instance | BindingFlags.NonPublic, null,
                [typeof(string), typeof(Action<string>), typeof(Action<Exception>), typeof(Func<bool>), typeof(bool)], null)!
                .Invoke([path, (Action<string>)lines.Enqueue, (Action<Exception>)errors.Enqueue, (Func<bool>)(() => false), true]);
            File.Move(path, path + ".old");
            File.WriteAllText(path, "zone:B\n", new UTF8Encoding(true));
            var readInitial = type.GetMethod("ReadInitialZone", BindingFlags.Instance | BindingFlags.NonPublic)!;
            // A superseded selection must stop scanning while leaving the pending handle usable for cleanup.
            Check(Invoke(readInitial, reader, root.ZoneChangeRegex, (Func<bool>)(() => true)) is null,
                "A cancelled initial zone search continued scanning.");
            // The replacement is shorter than the captured offset, making pathname reopening fail deterministically.
            var zone = (string?)Invoke(readInitial, reader, root.ZoneChangeRegex, (Func<bool>)(() => false));
            Check(zone == "original-long-zone", "Snapshot used replacement pathname rather than the owned old handle.");
            File.AppendAllText(path + ".old", "old-tail\n", new UTF8Encoding(false));
            Invoke(type.GetMethod("CompleteInitialSnapshot", BindingFlags.Instance | BindingFlags.NonPublic)!, reader);
            try
            {
                Invoke(readInitial, reader, root.ZoneChangeRegex, (Func<bool>)(() => false));
                throw new Exception("Initial read remained available after activation.");
            }
            catch (InvalidOperationException) { }
            Check(SpinWait.SpinUntil(() => lines.Count == 2, TimeSpan.FromSeconds(5)), "Activation did not drain old tail then replacement.");
            reader.Dispose();
            Check(lines.ToArray().SequenceEqual(["old-tail", "zone:B"]) && errors.IsEmpty,
                "Snapshot changed stream position or contaminated replacement delivery.");
            Check(host.Diagnostics.Item1 == 0, "Initial snapshot reported a spurious replacement/truncation error.");
            // Cancellation before activation returns without touching a released native handle.
            using var cancelled = (IDisposable)type.GetConstructor(BindingFlags.Instance | BindingFlags.NonPublic, null,
                [typeof(string), typeof(Action<string>), typeof(Action<Exception>), typeof(Func<bool>), typeof(bool)], null)!
                .Invoke([path, (Action<string>)lines.Enqueue, (Action<Exception>)errors.Enqueue, (Func<bool>)(() => false), true]);
            cancelled.Dispose();
            Check(Invoke(readInitial, cancelled, root.ZoneChangeRegex, (Func<bool>)(() => false)) is null,
                "Cancelled initial reader touched its disposed handle.");
        }
        finally { directory.Delete(true); }
    }

    private static object? Invoke(MethodInfo method, object target, params object?[] arguments)
    {
        try { return method.Invoke(target, arguments); }
        catch (TargetInvocationException error) when (error.InnerException is not null)
        {
            ExceptionDispatchInfo.Capture(error.InnerException).Throw();
            throw;
        }
    }
    private static void Check(bool value, string message)
    {
        if (!value) throw new InvalidOperationException(message);
    }
}
