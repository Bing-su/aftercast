using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.Loader;
using Advanced_Combat_Tracker;
using Aftercast.Bridge;

// Construct real pinned types without InitPlugin/StartMemory: no game, capture, updates or vendor disk writes.
internal static class VendorWriterLifecycleChecks
{
    internal static void Run()
    {
        Platform.Prepare();
        RunCase(false);
        RunCase(true);
        Console.WriteLine("Pinned vendor pending-queue shutdown and sticky unsafe-worker lifecycle checks passed.");
    }
    private static void RunCase(bool liveProducer)
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-writer-lifecycle-");
        var session = new PluginSession();
        using var release = new ManualResetEventSlim();
        var producer = new Thread(() => release.Wait()) { IsBackground = true, Name = "synthetic vendor producer" };
        try
        {
            session.Configure(directory.FullName);
            var path = Path.GetFullPath("binaries/FFXIV_ACT_Plugin_SDK_3.0.3.3/FFXIV_ACT_Plugin.dll");
            var plugin = session.Construct(path, "FFXIV_ACT_Plugin.FFXIV_ACT_Plugin");
            var context = AssemblyLoadContext.GetLoadContext(plugin.GetType().Assembly)!;
            var config = context.LoadFromAssemblyName(new("FFXIV_ACT_Plugin.Config"));
            var logfile = context.LoadFromAssemblyName(new("FFXIV_ACT_Plugin.Logfile"));
            var memory = context.LoadFromAssemblyName(new("FFXIV_ACT_Plugin.Memory"));
            var mediator = Activator.CreateInstance(config.GetType("FFXIV_ACT_Plugin.Config.SettingsMediator", true)!)!;
            var writer = Activator.CreateInstance(logfile.GetType("FFXIV_ACT_Plugin.Logfile.LogOutput", true)!, mediator, null, null)!;
            var collection = RuntimeHelpers.GetUninitializedObject(plugin.GetType().Assembly.GetType("FFXIV_ACT_Plugin.DataCollection", true)!);
            var scanner = RuntimeHelpers.GetUninitializedObject(memory.GetType("FFXIV_ACT_Plugin.Memory.ScanMemory", true)!);
            var monitor = RuntimeHelpers.GetUninitializedObject(plugin.GetType().Assembly.GetType("FFXIV_ACT_Plugin.MonitorNetwork", true)!);
            Set(scanner, "<ScanThread>k__BackingField", producer);
            Set(monitor, "<ScanThread>k__BackingField", new Thread(() => { }));
            Set(collection, "_logOutput", writer);
            Set(collection, "_settingsMediator", mediator);
            Set(collection, "_scanMemory", scanner);
            Set(collection, "_monitor", monitor);
            Set(collection, "_tokenSource", new CancellationTokenSource());
            var field = collection.GetType().GetField("_scanPackets", BindingFlags.Instance | BindingFlags.NonPublic)!;
            var packets = DispatchProxy.Create(field.FieldType, typeof(WriterPacketProxy));
            field.SetValue(collection, packets);
            Set(plugin, "_dataCollection", collection);
            var write = writer.GetType().GetMethod("WriteLine")!;
            var kind = logfile.GetType("FFXIV_ACT_Plugin.Logfile.LogMessageType", true)!;
            write.Invoke(writer, new[] { Enum.ToObject(kind, 0), (object)DateTime.UnixEpoch, "synthetic queued record" });
            if (liveProducer) producer.Start();
            var id = session.Add(path, plugin);
            var records = (Dictionary<long, PluginRecord>)typeof(PluginSession)
                .GetField("plugins", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(session)!;
            var record = records[id];
            // Supply only host-owned lifecycle metadata, without ever invoking the real collector initializer.
            record.State = PluginState.Initialized;
            record.Controls = (new(), new(), new(), new());
            typeof(PluginSession).GetMethod("CaptureWriter", BindingFlags.Instance | BindingFlags.NonPublic)!
                .Invoke(session, new object[] { record });
            var error = Reject(() => session.Deinitialize(id));
            Check(error.Contains("1 pending record(s)", StringComparison.Ordinal), "Pending queue was silently discarded");
            Check(session.Status(id).State == (int)PluginState.Stopped, "Instance must remain terminal");
            if (liveProducer)
            {
                Check(error.Contains("cannot be reused", StringComparison.Ordinal), "Live producer did not poison worker");
                Check(Reject(() => session.Construct(path, null)).Contains("cannot be reused", StringComparison.Ordinal), "Construct side effects were allowed");
                Check(Reject(() => session.Initialize(id)).Contains("cannot be reused", StringComparison.Ordinal), "Old instance was reusable");
            }
            else
            {
                _ = session.Construct(path, "FFXIV_ACT_Plugin.FFXIV_ACT_Plugin");
                // Pending queue alone does not imply a producer is still running; no replay is attempted.
            }
            release.Set();
            if (liveProducer) Check(producer.Join(1500), "Synthetic producer leaked");
            var oldRoot = ActGlobals.oFormActMain!;
            session.Stop();
            if (liveProducer)
            {
                Check(Reject(() => session.Configure(directory.FullName)).Contains("cannot be reused", StringComparison.Ordinal), "Stop reset unsafe process state");
                var nextDirectory = Directory.CreateDirectory(Path.Combine(directory.FullName, "next-host"));
                using var next = new Aftercast.ActCompat.HostEnvironment(nextDirectory.FullName);
                var errors = oldRoot.GetType().GetProperty("Errors", BindingFlags.Instance | BindingFlags.NonPublic)!;
                var before = ((Tuple<long, string>)errors.GetValue(oldRoot)!).Item1;
                mediator.GetType().GetMethod("AddExceptionMessage")!.Invoke(mediator, new object[]
                    { DateTime.UtcNow, "FFXIV_ACT_Plugin.LogOutput Error writing to log file. synthetic late callback" });
                Check(((Tuple<long, string>)errors.GetValue(oldRoot)!).Item1 == before + 1, "Late warning lost its original Host");
                Check(next.Diagnostics.Item1 == 0, "Late warning leaked into replacement ActGlobals");
            }
        }
        finally
        {
            release.Set();
            if (producer.IsAlive) producer.Join(1500);
            session.Stop();
            directory.Delete(true);
        }
    }
    private static void Set(object instance, string name, object value) =>
        (instance.GetType().GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)
        ?? throw new InvalidOperationException("Pinned fixture field changed: " + name)).SetValue(instance, value);
    private static string Reject(Action action)
    {
        try { action(); }
        catch (InvalidOperationException error) { return error.ToString(); }
        catch (AggregateException error) { return error.ToString(); }
        throw new InvalidOperationException("Expected lifecycle error");
    }
    private static void Check(bool condition, string message) { if (!condition) throw new InvalidOperationException(message); }
}

public class WriterPacketProxy : DispatchProxy
{
    protected override object? Invoke(MethodInfo? method, object?[]? args) => method!.Name == "StopScan" ? null :
        throw new NotSupportedException("Unexpected synthetic packet operation: " + method.Name);
}
