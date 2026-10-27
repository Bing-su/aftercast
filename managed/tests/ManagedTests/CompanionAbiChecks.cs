using System.Collections.Concurrent;
using System.Globalization;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.Bridge;

internal static class CompanionAbiChecks
{
    private const int Size = 32 + 512 * 1024;
    private static void Check(bool value, string message)
    {
        if (!value) throw new Exception("Companion ABI: " + message + " / " + Program.Diagnostic());
    }
    private sealed class Plugin : IActPluginV1
    {
        private FormActMain root = null!;
        internal readonly ConcurrentQueue<(bool Import, string Name, string Line)> Seen = new();
        internal readonly ManualResetEventSlim External = new();
        public void InitPlugin(TabPage tab, Label status)
        {
            root = ActGlobals.oFormActMain!;
            root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
            root.LastKnownTime = DateTime.UnixEpoch;
            root.BeforeLogLineRead += Read;
        }
        private void Read(bool import, LogLineEventArgs info)
        {
            Seen.Enqueue((import, info.companionLogName, info.originalLogLine));
            if (info.originalLogLine == "3|external") { root.OpenLog(false, false); External.Set(); }
        }
        public void DeInitPlugin() { root.BeforeLogLineRead -= Read; External.Dispose(); }
    }
    internal static void Run(long session)
    {
        var memory = Marshal.AllocHGlobal(Size);
        var directory = Directory.CreateTempSubdirectory("aftercast-companion-abi-");
        var shared = ((Lazy<PluginSession>)typeof(EntryPoint).GetField("plugins", BindingFlags.NonPublic | BindingFlags.Static)!.GetValue(null)!).Value;
        try
        {
            void Reset(int command, string json)
            {
                Marshal.Copy(new byte[32], 0, memory, 32);
                Marshal.WriteInt32(memory, 0, Size); Marshal.WriteInt32(memory, 4, 1); Marshal.WriteInt64(memory, 8, session);
                Marshal.WriteInt32(memory, 16, command);
                var bytes = Encoding.UTF8.GetBytes(json); Marshal.WriteInt32(memory, 20, bytes.Length); Marshal.Copy(bytes, 0, memory + 32, bytes.Length);
            }
            void Reject(int command, string json, Action? mutate = null, int? size = null)
            {
                Reset(command, json); mutate?.Invoke();
                Check(EntryPoint.CompanionCommand(memory, size ?? Size) == -1, "malformed request accepted");
            }
            JsonElement Call(int command, object request)
            {
                Reset(command, JsonSerializer.Serialize(request, HostJson.Options));
                Check(EntryPoint.CompanionCommand(memory, Size) == 0, "valid request failed");
                var length = Marshal.ReadInt32(memory, 24);
                Check(length is >= 2 and <= 512 * 1024, "response bounds");
                var bytes = new byte[length]; Marshal.Copy(memory + 32, bytes, 0, length);
                using var document = JsonDocument.Parse(bytes); return document.RootElement.Clone();
            }
            Check(EntryPoint.CompanionCommand(0, Size) == -1, "null capacity accepted");
            Reject(5, "{\"owner\":1}", size: Size - 1);
            foreach (var (offset, value) in new[] { (0, 32), (4, 2), (16, 0), (16, 7), (20, 0), (20, 131073), (24, 1), (28, 1) })
                Reject(5, "{\"owner\":1}", () => Marshal.WriteInt32(memory, offset, value));
            Reject(5, "{\"owner\":1}", () => Marshal.WriteInt64(memory, 8, session + 1));
            Reject(5, "{}", () => Marshal.WriteByte(memory, 32, 255));
            foreach (var json in new[] { "[]", "null", "{\"owner\":1,\"owner\":2}", "{\"owner\":1,\"extra\":0}", "{}", "{\"owner\":1,\"companions\":[]}" }) Reject(5, json);
            var main = Path.Combine(directory.FullName, "main.log");
            var sideDirectory = Directory.CreateDirectory(Path.Combine(directory.FullName, "CompanionLogs"));
            var side = Path.Combine(sideDirectory.FullName, "main.side.log");
            File.WriteAllText(main, "0|main\n1|end\n"); File.WriteAllText(side, "1|side\n2|excluded\n");
            var valid = JsonSerializer.Serialize(new CompanionRequest(Path: main, Companions: [new("side")]), HostJson.Options);
            Reject(1, valid.Replace("side", "bad.name", StringComparison.Ordinal));
            Reject(1, JsonSerializer.Serialize(new CompanionRequest(Path: main, Text: new(1), Companions: []), HostJson.Options));
            Reject(1, JsonSerializer.Serialize(new CompanionRequest(Path: main, StartTicks: 1, EndTicks: 0, Companions: []), HostJson.Options));
            Reject(4, JsonSerializer.Serialize(new CompanionRequest(Path: main, Companions: [], ClockWaitMilliseconds: 0), HostJson.Options));
            Reject(1, valid); // Validation succeeds, but no parser may be implicitly initialized.
            shared.Configure(directory.FullName);
            var plugin = new Plugin(); shared.Initialize(shared.Add(main, plugin));
            var request = new CompanionRequest(Path: main, Companions: [new("side")]);
            var owner = Call(1, request).GetProperty("owner").GetUInt64();
            request.Companions![0] = new("missing"); // Native bytes and admitted options no longer borrow caller objects.
            Reject(2, JsonSerializer.Serialize(new CompanionRequest(Owner: owner + 1), HostJson.Options));
            JsonElement result;
            do { result = Call(2, new CompanionRequest(Owner: owner)); if (result.GetProperty("pending").GetBoolean()) Thread.Sleep(1); }
            while (result.GetProperty("pending").GetBoolean());
            Check(result.GetProperty("result").GetProperty("recordsDispatched").GetInt64() == 3 &&
                result.GetProperty("result").GetProperty("companionHeadsExcluded").GetInt64() == 1 && plugin.Seen.Any(row => row.Name == "side"), "owned import result/name/exclusion");
            Reject(2, JsonSerializer.Serialize(new CompanionRequest(Owner: owner), HostJson.Options));
            var next = Call(1, new CompanionRequest(Path: main, Companions: [new("side")])).GetProperty("owner").GetUInt64();
            Reject(3, JsonSerializer.Serialize(new CompanionRequest(Owner: owner), HostJson.Options));
            Check(Call(3, new CompanionRequest(Owner: next)).GetProperty("cancelled").GetBoolean(), "owned cancel");
            var live = Call(4, new CompanionRequest(Path: main, Companions: [new("side")], ClockWaitMilliseconds: 1000));
            var liveOwner = live.GetProperty("owner").GetUInt64();
            Check(live.GetProperty("sources").GetArrayLength() == 2 && live.GetProperty("clockWaitMilliseconds").GetInt32() == 1000, "live snapshot");
            Reject(4, JsonSerializer.Serialize(new CompanionRequest(Path: main, Companions: []), HostJson.Options));
            Check(Call(5, new CompanionRequest(Owner: liveOwner)).GetProperty("owner").GetUInt64() == liveOwner, "invalid open replaced live owner");
            var plain = Call(4, new CompanionRequest(Owner: liveOwner, Path: main, Companions: [], Text: new()));
            var plainOwner = plain.GetProperty("owner").GetUInt64();
            Check(plain.GetProperty("sources").GetArrayLength() == 1, "single-source live default");
            Reject(5, JsonSerializer.Serialize(new CompanionRequest(Owner: liveOwner), HostJson.Options));
            File.AppendAllText(main, "3|external\n");
            Check(SpinWait.SpinUntil(() => { shared.Pump(); return plugin.External.IsSet; }, TimeSpan.FromSeconds(5)), "external plugin reopen");
            Reject(5, JsonSerializer.Serialize(new CompanionRequest(Owner: plainOwner), HostJson.Options));
            var fresh = Call(4, new CompanionRequest(Path: main, Companions: [new("side")])).GetProperty("owner").GetUInt64();
            var lockedPath = Path.Combine(sideDirectory.FullName, "main.zlocked.log"); File.WriteAllText(lockedPath, "");
            using (var locked = new FileStream(lockedPath, FileMode.Open, FileAccess.ReadWrite, FileShare.None))
            {
                Reject(4, JsonSerializer.Serialize(new CompanionRequest(Owner: fresh, Path: main, Companions: [new("side"), new("zlocked")]), HostJson.Options));
                Reject(5, JsonSerializer.Serialize(new CompanionRequest(Owner: fresh), HostJson.Options));
                using var exclusive = new FileStream(main, FileMode.Open, FileAccess.ReadWrite, FileShare.None);
            }
            var retry = Call(4, new CompanionRequest(Path: main, Companions: [new("side")])).GetProperty("owner").GetUInt64();
            Check(Call(6, new CompanionRequest(Owner: retry)).GetBoolean(), "close live selection");
            Reject(5, JsonSerializer.Serialize(new CompanionRequest(Owner: retry), HostJson.Options));
            Console.WriteLine("Companion ABI bounds/JSON/session, owned import/cancel, live snapshots, external revision and partial-open stale handles passed.");
        }
        finally { shared.Stop(); Marshal.FreeHGlobal(memory); directory.Delete(true); }
    }
}
