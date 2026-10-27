using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Bridge;
using Aftercast.Testing;

internal static class HistorySourceAbiChecks
{
    internal static void Run(long session, PluginSession shared, string directory)
    {
        const int size = 32 + 4 * 1024 * 1024;
        var memory = Marshal.AllocHGlobal(size);
        void Check(bool value, string message) { if (!value) throw new Exception("Source ABI: " + message + " / " + Program.Diagnostic()); }
        JsonElement Call(int command, object request, bool reject = false, bool allowFailure = false)
        {
            var input = JsonSerializer.SerializeToUtf8Bytes(request, HostJson.Options);
            Marshal.Copy(new byte[32], 0, memory, 32); Marshal.WriteInt32(memory, 0, size); Marshal.WriteInt32(memory, 4, 1);
            Marshal.WriteInt64(memory, 8, session); Marshal.WriteInt32(memory, 16, command); Marshal.WriteInt32(memory, 20, input.Length);
            Marshal.Copy(input, 0, memory + 32, input.Length);
            var result = EntryPoint.HistoryOperation(memory, size);
            if (reject) { Check(result == -1, "invalid source request accepted"); return default; }
            if (allowFailure && result == -1) return default;
            Check(result == 0, "source request failed");
            var length = Marshal.ReadInt32(memory, 24); Check(length is > 0 and <= 4 * 1024 * 1024, "reply bound");
            var bytes = new byte[length]; Marshal.Copy(memory + 32, bytes, 0, length);
            using var document = JsonDocument.Parse(bytes); return document.RootElement.Clone();
        }
        JsonElement Complete(int command, ulong owner)
        {
            var until = DateTime.UtcNow.AddSeconds(10);
            while (DateTime.UtcNow < until)
            {
                shared.Pump(); var reply = Call(command, new HistorySourceRequest(Owner: owner));
                if (!reply.GetProperty("pending").GetBoolean()) return reply;
                Thread.Sleep(1);
            }
            throw new Exception("Source ABI task timed out");
        }
        try
        {
            var root = ActGlobals.oFormActMain!; CombatFixture.ConfigureCategories();
            var path = Path.Combine(directory, "source-history.log"); File.WriteAllText(path, "1|saved\n3|end\n"); root.LogFilePath = path;
            root.SetEncounter(DateTime.UnixEpoch.AddSeconds(1), "Source", "Target");
            root.AddCombatAction(new(1, false, "", new Dnum(1), DateTime.UnixEpoch.AddSeconds(1), 1, "Strike", "Source", "Physical", "Target"));
            root.SetEncounter(DateTime.UnixEpoch.AddSeconds(3), "Source", "Target");
            root.AddCombatAction(new(1, false, "", new Dnum(1), DateTime.UnixEpoch.AddSeconds(3), 3, "Strike", "Source", "Physical", "Target"));
            shared.EndCombat();
            shared.ArchiveOperation(3, "", 0, false);
            shared.BeginHistory(1, new HistoryRequest(Query: new())); byte[]? page;
            do { shared.Pump(); page = shared.AdvanceHistory(); if (page is null) Thread.Sleep(1); } while (page is null);
            using var document = JsonDocument.Parse(page); var catalog = document.RootElement;
            var selection = new HistorySelection(catalog.GetProperty("indexRevision").GetString()!, [catalog.GetProperty("items")[0].GetProperty("id").GetString()!]);
            var query = new HistorySourceQuery([path], []);
            var owner = Call(3, new HistorySourceRequest(Selection: selection, Query: query)).GetProperty("owner").GetUInt64();
            Check(Complete(4, owner).GetProperty("snapshot").GetProperty("candidates").GetArrayLength() == 1, "metadata catalog");
            Call(4, new HistorySourceRequest(Owner: owner + 1), true);
            var failed = Call(6, new HistorySourceRequest(Owner: owner, Path: path, Companions: [], MaximumBytes: 1)).GetProperty("owner").GetUInt64();
            // Failed capture remains owned and observable, then release retires it even when reporting its error.
            var failureDeadline = DateTime.UtcNow.AddSeconds(10);
            while (true)
            {
                var pending = Call(7, new HistorySourceRequest(Owner: failed), allowFailure: true);
                if (pending.ValueKind == JsonValueKind.Undefined) break;
                Check(pending.GetProperty("pending").GetBoolean() && DateTime.UtcNow < failureDeadline, "budget failure did not complete");
                shared.Pump(); Thread.Sleep(1);
            }
            Call(8, new HistorySourceRequest(Owner: failed), true);
            Call(7, new HistorySourceRequest(Owner: failed), true);
            var capture = Call(6, new HistorySourceRequest(Owner: owner, Path: path, Companions: [])).GetProperty("owner").GetUInt64();
            var ready = Complete(7, capture); Check(ready.GetProperty("files")[0].GetProperty("sha256").GetString()!.Length == 64, "verified image metadata");
            Call(5, new HistorySourceRequest(Owner: owner), true);
            File.WriteAllText(path, "9|replacement\n");
            var replay = Call(9, new HistorySourceRequest(Owner: capture)).GetProperty("owner").GetUInt64();
            Call(9, new HistorySourceRequest(Owner: capture), true);
            var deadline = DateTime.UtcNow.AddSeconds(10);
            JsonElement final = default;
            while (DateTime.UtcNow < deadline)
            {
                shared.Pump();
                using var polled = JsonDocument.Parse(shared.CompanionOperation(2, new CompanionRequest(Owner: replay)));
                if (!polled.RootElement.GetProperty("pending").GetBoolean()) { final = polled.RootElement.Clone(); break; }
                Thread.Sleep(1);
            }
            Check(final.ValueKind != JsonValueKind.Undefined && final.GetProperty("result").GetProperty("recordsDispatched").GetInt64() == 2,
                "frozen saved rows were not replayed");
            Call(5, new HistorySourceRequest(Owner: owner)); Call(4, new HistorySourceRequest(Owner: owner), true);
            Console.WriteLine("Source ABI proposal/capture owners, bounded failure release, frozen replay and one-shot transfer passed.");
        }
        finally { Marshal.FreeHGlobal(memory); }
    }
}
