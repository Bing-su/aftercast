using System.Diagnostics;
using System.Reflection;
using System.Runtime.Loader;
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
    foreach (var scenario in new[] { "success", "io-open", "cancel-after-flush" })
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
    if (scenario == "io-open")
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
    var mediator = Proxy(config.GetType("FFXIV_ACT_Plugin.Config.ISettingsMediator", true)!, (method, values) =>
    {
        if (method.Name == "get_DataCollectionSettings") return settings;
        if (method.Name == "AddExceptionMessage")
        {
            lock (privateMessages) privateMessages.Add((string)values![1]!);
            return null;
        }
        throw new NotSupportedException("Unexpected settings call: " + method.Name);
    });
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
        var result = ((Delegate)values![1]!).DynamicInvoke();
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
    var thread = (Thread)writerType.GetProperty("ScanThread")!.GetValue(writer)!;
    var write = writerType.GetMethod("WriteLine")!;
    var messageType = logfile.GetType("FFXIV_ACT_Plugin.Logfile.LogMessageType", true)!;
    var time = DateTime.UnixEpoch;
    var payloads = new[] { "First", "Unicode 한글", "Nul\0payload" };
    foreach (var payload in payloads)
        write.Invoke(writer, new[] { Enum.ToObject(messageType, 0), (object)time, payload });
    thread.Start(cancellation.Token);
    try
    {
        if (scenario == "cancel-after-flush")
        {
            // A deterministic cancellation point follows genuine Flush and precedes the vendor's next loop test.
            if (!patched)
            {
                thread.Join();
                throw new InvalidOperationException("Original cancellation unexpectedly stopped; update the known-failure baseline.");
            }
            Check(SpinWait.SpinUntil(() => cancellation.IsCancellationRequested, 2000), "Patched writer did not reach Flush cancellation.");
        }
        else if (scenario == "io-open")
        {
            Check(SpinWait.SpinUntil(() => { lock (privateMessages) return privateMessages.Count != 0; }, 2000), "No private I/O diagnostic.");
            lock (privateMessages) Check(privateMessages.All(message => message.Contains("LogOutput Error writing to log file.", StringComparison.Ordinal)), "Wrong I/O sink message.");
            lock (hostMessages) Check(hostMessages.Count == 0, "I/O error unexpectedly used host error sink.");
        }
        else
        {
            Check(SpinWait.SpinUntil(() => Volatile.Read(ref flushCount) == payloads.Length, 2000), "Original writer did not flush all records.");
            Check(followed.Count == 1, "Expected one original follow callback.");
            var bytes = File.ReadAllBytes(followed[0]);
            var lines = new UTF8Encoding(false, true).GetString(bytes).Split('\n', StringSplitOptions.RemoveEmptyEntries);
            Check(lines.Length == payloads.Length && !bytes.AsSpan().StartsWith(Encoding.UTF8.Preamble), "Unexpected encoding/BOM/record count.");
            for (var index = 0; index < lines.Length; index++)
            {
                var body = $"00|{time:O}|{payloads[index].Replace('\0', ' ')}";
                var hashInput = body + "|" + (index + 1);
                var hash = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(hashInput)).AsSpan(0, 8)).ToLowerInvariant();
                Check(lines[index].TrimEnd('\r') == body + "|" + hash, "Original bytes/order/hash differed: " + lines[index]);
            }
            // Observe the real empty-queue wait, avoiding the separate known cancellation race after Flush.
            Check(SpinWait.SpinUntil(() => (thread.ThreadState & System.Threading.ThreadState.WaitSleepJoin) != 0, 2000),
                "Original writer did not enter its empty-queue wait.");
        }
    }
    finally
    {
        cancellation.Cancel();
        Check(thread.Join(1500), "Original writer worker did not stop; parent watchdog must reap this process.");
    }
    var pending = ((System.Collections.ICollection)writerType.GetField("_LogQueue", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(writer)!).Count;
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
        hostErrors = hostMessages.Count
    });
    return 0;
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
