using System.Diagnostics;
using System.Reflection;
using System.Runtime.Loader;
using System.Runtime.ExceptionServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Aftercast.Bridge;

// Only this separate test process loads copied vendor assemblies; no game or capture workers are constructed.
if (args.Length > 0 && args[0] == "child") return RunChild(args[1], args[2], args[3], bool.Parse(args[4]));
if (args.Length is < 1 or > 2 || args.Length == 2 && args[1] != "--patched")
    throw new ArgumentException("Pass the pinned SDK deployment directory and optionally --patched.");
var patched = args.Length == 2;
var temporary = Directory.CreateTempSubdirectory("aftercast-original-writer-");
try
{
    var deployment = Directory.CreateDirectory(Path.Combine(temporary.FullName, "deployment"));
    foreach (var (name, expected) in new[]
    {
        ("Logfile", "2f4faa819df144ca70a17c17e79e45c9cdecf413c98c12fc42b51d73af10b5e7"),
        ("Common", "9c3ce07598b6e77a45a6e346bf6c4efbbb09b9cc61a32534f23bc5adcdb03cb1"),
        ("Config", "f965992b4f520ae9aabc2158f925ae4044be647113bc1e7770abb9a6e3b20b2e")
    })
    {
        File.Copy(Path.Combine(args[0], $"SDK\\FFXIV_ACT_Plugin.{name}.dll"),
            Path.Combine(deployment.FullName, $"FFXIV_ACT_Plugin.{name}.dll"));
        Check(Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(Path.Combine(deployment.FullName,
            $"FFXIV_ACT_Plugin.{name}.dll")))) == expected, $"{name} does not match pinned SDK 3.0.3.3 evidence.");
    }
    if (patched)
    {
        var path = Path.Combine(deployment.FullName, "FFXIV_ACT_Plugin.Logfile.dll");
        File.WriteAllBytes(path, LinuxPluginCompatibility.RewriteLogfile(File.ReadAllBytes(path)));
    }
    foreach (var scenario in new[] { "success", "io-open", "open-recovery", "write-failure", "flush-failure", "diagnostic-failure", "cancel-before-start", "cancel-after-flush", "dispose-enqueue", "live-producer", "callback-replacement", "restart" })
        RunIsolated(scenario, deployment.FullName, temporary.FullName, patched);
    return 0;
}
finally { temporary.Delete(true); }

static void Check(bool condition, string message)
{
    if (!condition) throw new InvalidOperationException(message);
}

static void Report(object value) => Console.WriteLine(JsonSerializer.Serialize(value));

static void RunIsolated(string scenario, string deployment, string temporary, bool patched)
{
    var output = Path.Combine(temporary, scenario);
    Directory.CreateDirectory(output);
    var start = new ProcessStartInfo("dotnet") { RedirectStandardOutput = true, RedirectStandardError = true };
    foreach (var argument in new[] { Assembly.GetExecutingAssembly().Location, "child", scenario, deployment, output, patched.ToString() })
        start.ArgumentList.Add(argument);
    using var process = Process.Start(start)!;
    var stdout = process.StandardOutput.ReadToEndAsync();
    var stderr = process.StandardError.ReadToEndAsync();
    var exited = process.WaitForExit(5000);
    if (!exited)
    {
        // Kill the entire child tree before inspecting output; no surviving worker is accepted as a successful stop.
        process.Kill(entireProcessTree: true);
        Check(process.WaitForExit(5000), "Watchdog could not reap the original writer process.");
    }
    var text = stdout.GetAwaiter().GetResult();
    var errors = stderr.GetAwaiter().GetResult();
    if (scenario == "cancel-after-flush" && !patched)
    {
        Check(!exited && text.Contains("cancellation-requested-after-flush", StringComparison.Ordinal),
            $"Expected bounded reproduction of the original cancellation spin: {text} {errors}");
        Report(new { scenario, status = "known-failure", childReaped = true, detail = "Original writer did not stop after cancellation following flush; watchdog killed its process." });
    }
    else
    {
        Check(exited && process.ExitCode == 0, $"Writer case {scenario} failed: {text} {errors}");
        Console.Write(text);
    }
}

static int RunChild(string scenario, string deployment, string output, bool patched)
{
    AssemblyLoadContext.Default.Resolving += (_, name) =>
    {
        var path = Path.Combine(deployment, name.Name + ".dll");
        return File.Exists(path) ? AssemblyLoadContext.Default.LoadFromAssemblyPath(path) : null;
    };
    var logfile = AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.Combine(deployment, "FFXIV_ACT_Plugin.Logfile.dll"));
    var common = AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.Combine(deployment, "FFXIV_ACT_Plugin.Common.dll"));
    var config = AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.Combine(deployment, "FFXIV_ACT_Plugin.Config.dll"));
    using var cancellation = new CancellationTokenSource();
    var settings = Activator.CreateInstance(config.GetType("FFXIV_ACT_Plugin.Config.DataCollectionSettingsEventArgs", true)!)!;
    var folder = output;
    if (scenario is "io-open" or "open-recovery")
    {
        folder = Path.Combine(output, "regular-file-not-directory");
        File.WriteAllText(folder, "block directory creation");
    }
    settings.GetType().GetField("LogFileFolder")!.SetValue(settings, folder);
    var privateMessages = new List<string>();
    var hostMessages = new List<string>();
    var followed = new List<string>();
    var currentPath = "";
    var flushCount = 0;
    var mediator = Activator.CreateInstance(config.GetType("FFXIV_ACT_Plugin.Config.SettingsMediator", true)!)!;
    mediator.GetType().GetProperty("DataCollectionSettings")!.SetValue(mediator, settings);
    Action<DateTime, string> originalCallback = (_, message) => { lock (privateMessages) privateMessages.Add(message); };
    mediator.GetType().GetProperty("ProcessException")!.SetValue(mediator, originalCallback);
    var observedMessages = new List<string>();
    var injected = false;
    var act = Proxy(common.GetType("FFXIV_ACT_Plugin.Common.IActWrapper", true)!, (method, values) =>
    {
        switch (method.Name)
        {
            case "IsMainFormVisible": return true;
            case "get_LogFilePath": return currentPath;
            case "set_LogFilePath": currentPath = (string)values![0]!; return null;
            case "set_LogFileFilter": Check((string)values![0]! == "Network_*.log", "Original filter changed."); return null;
            case "OpenLog":
                Check((bool)values![0]! && !(bool)values[1]!, "Unexpected original OpenLog flags.");
                Check(File.Exists(currentPath), "Original follow callback preceded empty file creation.");
                followed.Add(currentPath); return null;
            case "WriteErrorLog": lock (hostMessages) hostMessages.Add(values![0]!.ToString()!); return null;
            default: throw new NotSupportedException("Unexpected ACT wrapper call: " + method.Name);
        }
    });
    var benchmark = Proxy(common.GetType("FFXIV_ACT_Plugin.Common.IBenchmarkRepository", true)!, (method, values) =>
    {
        Check(method.Name == "Measure", "Unexpected benchmark method.");
        var operation = (Delegate)values![1]!;
        if (!injected && (string)values[0]! == "LogLineWrite" &&
            scenario is "write-failure" or "flush-failure" or "diagnostic-failure")
        {
            FaultingStream.Install(operation, scenario != "flush-failure");
            injected = true;
        }
        var result = Invoke(operation);
        if ((string)values[0]! == "LogLineFlush")
        {
            Interlocked.Increment(ref flushCount);
            if (scenario == "cancel-after-flush")
            {
                cancellation.Cancel();
                Report(new { scenario, status = "cancellation-requested-after-flush" });
            }
        }
        return result;
    });
    var writerType = logfile.GetType("FFXIV_ACT_Plugin.Logfile.LogOutput", true)!;
    var writer = Activator.CreateInstance(writerType, mediator, act, benchmark)!;
    using var producerRelease = new ManualResetEventSlim();
    var producer = new Thread(() => producerRelease.Wait()) { IsBackground = true, Name = "synthetic producer" };
    if (scenario == "live-producer") producer.Start();
    var observer = new VendorWriterObserver(writer, mediator,
        scenario == "live-producer" ? new[] { producer } : Array.Empty<Thread>(), message =>
        {
            if (scenario == "diagnostic-failure") throw new IOException("Synthetic diagnostic sink failure");
            lock (observedMessages) observedMessages.Add(message);
        });
    var thread = (Thread)writerType.GetProperty("ScanThread")!.GetValue(writer)!;
    var write = writerType.GetMethod("WriteLine")!;
    var messageType = logfile.GetType("FFXIV_ACT_Plugin.Logfile.LogMessageType", true)!;
    var time = DateTime.UnixEpoch;
    var payloads = new[] { "First", "Unicode 한글", "Nul\0payload" };
    foreach (var payload in payloads)
        write.Invoke(writer, new[] { Enum.ToObject(messageType, 0), (object)time, payload });
    if (scenario == "cancel-before-start") cancellation.Cancel();
    thread.Start(cancellation.Token);
    try
    {
        if (scenario == "cancel-before-start")
        {
            Check(thread.Join(1500), "Pre-canceled writer did not stop");
            Check(currentPath == "", "Pre-canceled writer unexpectedly opened a file");
        }
        else if (scenario == "cancel-after-flush")
        {
            // A deterministic cancellation point follows genuine Flush and precedes the vendor's next loop test.
            if (!patched)
            {
                thread.Join();
                throw new InvalidOperationException("Original cancellation unexpectedly stopped; update the known-failure baseline.");
            }
            Check(SpinWait.SpinUntil(() => cancellation.IsCancellationRequested, 2000), "Patched writer did not reach Flush cancellation.");
        }
        else if (scenario is "io-open" or "open-recovery")
        {
            Check(SpinWait.SpinUntil(() => { lock (privateMessages) return privateMessages.Count != 0; }, 2000), "No private I/O diagnostic.");
            lock (privateMessages) Check(privateMessages.All(message => message.Contains("LogOutput Error writing to log file.", StringComparison.Ordinal)), "Wrong I/O sink message.");
            lock (hostMessages) Check(hostMessages.Count == 0, "I/O error unexpectedly used host error sink.");
            lock (observedMessages) Check(observedMessages.Count != 0, "Observer failed to expose private I/O diagnostic");
            if (scenario == "open-recovery")
            {
                // Only remove this scenario's synthetic blocker, never a user log or directory.
                File.Delete(folder);
                Directory.CreateDirectory(folder);
                Check(SpinWait.SpinUntil(() => Volatile.Read(ref flushCount) == payloads.Length, 2000), "Open recovery did not retain queued records");
                VerifyRecords(currentPath, payloads, time, 0);
                WaitForIdle(thread);
            }
        }
        else
        {
            var failedOperation = scenario is "write-failure" or "flush-failure" or "diagnostic-failure";
            var expected = payloads.Length - (failedOperation ? 1 : 0);
            Check(SpinWait.SpinUntil(() => Volatile.Read(ref flushCount) == expected, 2000), "Writer did not finish the expected successful flushes.");
            Check(followed.Count == 1, "Expected one original follow callback.");
            var skip = scenario is "write-failure" or "diagnostic-failure" ? 1 : 0;
            VerifyRecords(followed[0], payloads, time, skip);
            if (failedOperation)
            {
                lock (privateMessages) Check(privateMessages.Count == 1, "Expected a private I/O failure");
                lock (observedMessages) Check(observedMessages.Count == (scenario == "diagnostic-failure" ? 0 : 1), "Observer sink mismatch");
            }
            WaitForIdle(thread);
        }
    }
    finally
    {
        cancellation.Cancel();
        Check(thread.Join(1500), "Original writer worker did not stop; parent watchdog must reap this process.");
    }
    if (scenario == "dispose-enqueue")
        new FinalLogProducer(() => write.Invoke(writer, new[] { Enum.ToObject(messageType, 0), (object)time, "final disposal record" })).Dispose();
    Action<DateTime, string> replacement = (_, _) => { };
    if (scenario == "callback-replacement") mediator.GetType().GetProperty("ProcessException")!.SetValue(mediator, replacement);
    var observation = observer.Finish(() => { }, TimeSpan.Zero);
    Check(observation.PendingRecords == (scenario switch { "io-open" or "cancel-before-start" => 3, "cancel-after-flush" => 2, "dispose-enqueue" => 1, _ => 0 }), "Wrong final pending count");
    Check(observation.IoErrors == privateMessages.Count, "Private failure count was lost");
    Check(observation.DiagnosticFailures == (scenario == "diagnostic-failure" ? 1 : 0), "Diagnostic failures were hidden");
    var reuse = new WorkerReusePolicy();
    reuse.Observe(observation);
    if (scenario == "live-producer")
    {
        Check(observation.RequiresRestart, "Live producer was accepted as stopped");
        MustReject(reuse.RequireReusable);
        producerRelease.Set();
        Check(producer.Join(1500), "Synthetic producer did not stop");
        var stopped = observer.Finish(() => { });
        Check(!stopped.RequiresRestart, "Stopped producer still appears live");
        reuse.Observe(stopped);
        MustReject(reuse.RequireReusable); // A later clean observation cannot un-poison this worker.
        new WorkerReusePolicy().RequireReusable();
    }
    else
    {
        reuse.RequireReusable();
        Check(ReferenceEquals(mediator.GetType().GetProperty("ProcessException")!.GetValue(mediator),
            scenario == "callback-replacement" ? replacement : originalCallback), "Callback restoration overwrote a later plugin callback");
    }
    if (scenario == "restart") RestartWriter(writerType, mediator, act, benchmark, messageType, time, currentPath, ref flushCount);
    var pending = observation.PendingRecords;
    // Correcting cancellation termination does not invent a vendor queue-drain guarantee.
    if (scenario == "cancel-after-flush") Check(pending == payloads.Length - 1, "Cancellation unexpectedly drained pending records.");
    Report(new
    {
        scenario,
        patched,
        status = "passed",
        flushes = flushCount,
        pendingRecords = pending,
        followedPath = currentPath,
        workerJoined = true,
        privateErrors = privateMessages.Count,
        hostErrors = hostMessages.Count,
        observedIoErrors = observation.IoErrors,
        diagnosticFailures = observation.DiagnosticFailures,
        activeThreads = observation.ActiveThreads,
        terminalStatus = observation.Failed ? "recording-incomplete-or-uncertain" : "no-observed-fault"
    });
    return 0;
}

static object? Invoke(Delegate operation)
{
    try { return operation.DynamicInvoke(); }
    catch (TargetInvocationException error) when (error.InnerException is not null)
    { ExceptionDispatchInfo.Capture(error.InnerException).Throw(); throw; }
}

static void MustReject(Action action)
{
    try { action(); }
    catch (InvalidOperationException error) when (error.Message.Contains("cannot be reused", StringComparison.Ordinal)) { return; }
    throw new InvalidOperationException("Unsafe worker reuse was accepted");
}

static void WaitForIdle(Thread thread) => Check(SpinWait.SpinUntil(() =>
    (thread.ThreadState & System.Threading.ThreadState.WaitSleepJoin) != 0, 2000), "Writer did not enter its empty-queue wait");

static void VerifyRecords(string path, string[] payloads, DateTime time, int skip)
{
    var bytes = File.ReadAllBytes(path);
    var lines = new UTF8Encoding(false, true).GetString(bytes).Split('\n', StringSplitOptions.RemoveEmptyEntries);
    Check(lines.Length == payloads.Length - skip && !bytes.AsSpan().StartsWith(Encoding.UTF8.Preamble), "Wrong encoding or failure-boundary record count");
    for (var index = skip; index < payloads.Length; index++)
    {
        var body = $"00|{time:O}|{payloads[index].Replace('\0', ' ')}";
        var hash = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(body + "|" + (index + 1))).AsSpan(0, 8)).ToLowerInvariant();
        Check(lines[index - skip].TrimEnd('\r') == body + "|" + hash, "Original record bytes/order/hash changed");
    }
}

static void RestartWriter(Type writerType, object mediator, object act, object benchmark, Type messageType, DateTime time, string path, ref int flushCount)
{
    var writer = Activator.CreateInstance(writerType, mediator, act, benchmark)!;
    var thread = (Thread)writerType.GetProperty("ScanThread")!.GetValue(writer)!;
    writerType.GetMethod("WriteLine")!.Invoke(writer, new[] { Enum.ToObject(messageType, 0), (object)time, "Restart" });
    using var cancellation = new CancellationTokenSource();
    thread.Start(cancellation.Token);
    try
    {
        Check(SpinWait.SpinUntil(() => File.ReadAllLines(path).Length == 4, 2000), "Restart failed to append exactly one record");
        WaitForIdle(thread);
    }
    finally { cancellation.Cancel(); Check(thread.Join(1500), "Restarted writer did not stop"); }
    var body = $"00|{time:O}|Restart";
    var hash = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(body + "|1")).AsSpan(0, 8)).ToLowerInvariant();
    Check(File.ReadAllLines(path)[3] == body + "|" + hash && flushCount == 4, "Restart changed vendor sequence semantics");
}

static object Proxy(Type contract, Func<MethodInfo, object?[]?, object?> call)
{
    var instance = DispatchProxy.Create(contract, typeof(WriterProxy));
    ((WriterProxy)instance).Call = call;
    return instance;
}

public class WriterProxy : DispatchProxy
{
    public Func<MethodInfo, object?[]?, object?> Call = null!;
    protected override object? Invoke(MethodInfo? method, object?[]? arguments) => Call(method!, arguments);
}

internal sealed class FinalLogProducer(Action write) : IDisposable
{
    public void Dispose() => write();
}
