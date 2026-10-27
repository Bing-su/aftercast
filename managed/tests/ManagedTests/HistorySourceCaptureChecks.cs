using System.Globalization;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using Aftercast.ActCompat;
using Advanced_Combat_Tracker;
using Aftercast.Testing;

internal static class HistorySourceCaptureChecks
{
    private static readonly DateTime Epoch = DateTime.UnixEpoch;
    private static void Check(bool value, string message) { if (!value) throw new Exception("Source capture: " + message); }
    private static void Reject<T>(Action action) where T : Exception
    { try { action(); } catch (T) { return; } throw new Exception("Expected " + typeof(T).Name); }
    private sealed class Fixture : IDisposable
    {
        internal readonly DirectoryInfo Directory = System.IO.Directory.CreateTempSubdirectory("aftercast-source-images-");
        internal readonly HostEnvironment Host;
        internal readonly FormActMain Root;
        internal string Main => Path.Combine(Directory.FullName, "main.log");
        internal string Side(string name) => Root.GetCompanionFilePath(Main, name);
        internal Fixture()
        {
            Host = new(Directory.FullName); Root = ActGlobals.oFormActMain!; CombatFixture.ConfigureCategories();
            Root.LogFilePath = Main;
            foreach (var second in new[] { 1, 3 })
            {
                Root.SetEncounter(Epoch.AddSeconds(second), "Source", "Target");
                Root.AddCombatAction(new(1, false, "", new Dnum(1), Epoch.AddSeconds(second), second, "Strike", "Source", "Physical", "Target"));
            }
            Host.EndCombat(); Root.ActiveZone.ActiveEncounter!.CharName = "Source"; Host.ArchiveOperation(3, "", 0, false);
            Root.GetDateTimeFromLog = line => Epoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
            var owner = Environment.CurrentManagedThreadId;
            Root.BeforeLogLineRead += (import, info) =>
            {
                Check(import && Environment.CurrentManagedThreadId == owner, "parser left its owner/import path");
                Root.SetEncounter(info.detectedTime, "Source", "Target");
                Root.AddCombatAction(new(1, false, "", new Dnum(long.Parse(info.logLine.Split('|')[1], CultureInfo.InvariantCulture)),
                    info.detectedTime, Root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
            };
            System.IO.Directory.CreateDirectory(Path.Combine(Directory.FullName, "CompanionLogs"));
            Root.LogFilePath = "live.log"; ActGlobals.charName = "LiveHero";
        }
        internal HistorySourceSnapshot Proposal()
        {
            var page = Host.ReadHistory(new());
            return Host.ReadHistorySources(new(page.IndexRevision, [page.Items.Single().Id]), new([Main], []));
        }
        internal void NoScratch() => Check(!System.IO.Directory.EnumerateFiles(Directory.FullName, ".aftercast-source-*.tmp").Any(), "scratch name survived cleanup");
        public void Dispose() { try { Host.Dispose(); } finally { Directory.Delete(true); } }
    }
    internal static void Run()
    {
        HostLifetimeChecks.RunBounded(FrozenReplay);
        HostLifetimeChecks.RunBounded(BudgetAndAdmission);
        HostLifetimeChecks.RunBounded(IntegrityAndPartialResults);
        HostLifetimeChecks.RunBounded(CancellationAndStop);
        HostLifetimeChecks.RunBounded(WriteAndFlushFailures);
        HostLifetimeChecks.RunBounded(ExactBudgetBoundaries);
        HostLifetimeChecks.RunBounded(() => FailedImportShutdown(false));
        HostLifetimeChecks.RunBounded(() => FailedImportShutdown(true));
        foreach (var mode in new[] { "cancel", "grow", "rewrite", "replace", "history", "stop" })
            HostLifetimeChecks.RunBounded(() => ChangeDuringCapture(mode));
        HostLifetimeChecks.RunBounded(ActiveImportStopAndCallbackGuard);
        HostLifetimeChecks.RunBounded(ProducerFailureDuringStop);
        HostLifetimeChecks.RunBounded(RangeCutoffIgnoresUndispatchedSuffix);
        Console.WriteLine("Captured source bytes, original-path-independent range/companion replay, integrity, limits, one-shot ownership and shutdown checks passed.");
    }
    private static ImportResult Drain(HostEnvironment host)
    {
        var limit = DateTime.UtcNow.AddSeconds(10);
        while (true)
        {
            var before = host.ImportProgress;
            if (host.AdvanceImport(false) is { } result) return result;
            Check(DateTime.UtcNow < limit, "import did not finish");
            if (before == host.ImportProgress) Thread.Sleep(1);
        }
    }
    private static void FrozenReplay()
    {
        using var f = new Fixture();
        File.WriteAllText(f.Main, "0|999\n1|10|한글\n3|20\n4|999\n", new UTF8Encoding(false));
        File.WriteAllText(f.Side("alpha"), "1|21|한글\n3|22\n6|999\n", Encoding.GetEncoding(949));
        File.WriteAllText(f.Side("beta"), "2|30|日本\n", Encoding.Unicode);
        var original = File.ReadAllBytes(f.Main); var snapshot = f.Proposal();
        using var image = f.Host.CaptureHistorySourceAsync(snapshot, f.Main, companions: [new("beta"), new("alpha", new(949))]).GetAwaiter().GetResult();
        Check(image.Range.Start == Epoch.AddSeconds(1) && image.Range.End == Epoch.AddSeconds(3), "history inclusive range");
        Check(image.Files.Count == 3 && image.Files[0].Sha256 == Convert.ToHexStringLower(SHA256.HashData(original)), "captured digest/count");
        Check(File.ReadAllBytes(f.Main).SequenceEqual(original), "capture wrote original bytes");
        Reject<InvalidOperationException>(() => f.Host.ImportLog(f.Main));
        Reject<InvalidOperationException>(() => f.Host.ArchiveOperation(3, "", 0, false));
        File.Move(f.Main, f.Main + ".old"); File.WriteAllText(f.Main, "2|9999\n");
        File.Delete(f.Side("alpha")); File.Delete(f.Side("beta")); System.IO.Directory.Delete(Path.GetDirectoryName(f.Side("alpha"))!);
        f.Root.LogEncoding = Encoding.ASCII;
        var paths = new List<OriginalLogRecord>(); f.Host.OriginalLogRecordRead += paths.Add;
        var characters = new List<string?>(); f.Root.OnLogLineRead += (_, _) => characters.Add(ActGlobals.charName);
        f.Host.BeginCapturedHistoryImport(image); image.Dispose(); // Observer disposal cannot close transferred readers.
        var result = Drain(f.Host);
        Check(result.RecordsDispatched == 5 && result.Damage == 103 && result.CompanionHeadsExcluded == 1, $"frozen aggregate/range: {result}; records={string.Join(",", paths.Select(record => record.CompanionLogName + ":" + record.Line))}");
        Check(paths.All(record => record.Path == (record.CompanionLogName.Length == 0 ? f.Main : f.Side(record.CompanionLogName))), "scratch path leaked to plugin");
        Check(characters.All(name => name == "Source") && ActGlobals.charName == "LiveHero", "history character/live context restoration");
        Check(image.State == HistorySourceCaptureState.Consumed, "capture was not one shot");
        Reject<InvalidOperationException>(() => f.Host.BeginCapturedHistoryImport(image)); f.NoScratch();
    }
    private static void BudgetAndAdmission()
    {
        using var f = new Fixture(); File.WriteAllText(f.Main, "1|10\n"); var snapshot = f.Proposal();
        Reject<ArgumentOutOfRangeException>(() => f.Host.CaptureHistorySourceAsync(snapshot, f.Main, maximumBytes: 0));
        Reject<InvalidDataException>(() => f.Host.CaptureHistorySourceAsync(snapshot, f.Main, maximumBytes: 1).GetAwaiter().GetResult());
        File.AppendAllText(f.Main, "3|20\n");
        Reject<IOException>(() => f.Host.CaptureHistorySourceAsync(snapshot, f.Main).GetAwaiter().GetResult());
        snapshot = f.Proposal();
        File.WriteAllText(f.Side("alpha"), "1|10\n");
        Reject<FileNotFoundException>(() => f.Host.CaptureHistorySourceAsync(snapshot, f.Main, companions: [new("alpha"), new("zmissing")]).GetAwaiter().GetResult());
        using (var probe = new FileStream(f.Main, FileMode.Open, FileAccess.ReadWrite, FileShare.None)) { }
        using (var probe = new FileStream(f.Side("alpha"), FileMode.Open, FileAccess.ReadWrite, FileShare.None)) { }
        using var capture = f.Host.CaptureHistorySourceAsync(snapshot, f.Main).GetAwaiter().GetResult();
        var index = Path.Combine(f.Directory.FullName, "history/index.xml"); var saved = File.ReadAllBytes(index); File.AppendAllText(index, " ");
        try { Reject<InvalidOperationException>(() => f.Host.BeginCapturedHistoryImport(capture)); Check(capture.State == HistorySourceCaptureState.Ready, "stale index consumed capture"); }
        finally { File.WriteAllBytes(index, saved); }
        capture.Dispose(); Reject<InvalidOperationException>(() => f.Host.BeginCapturedHistoryImport(capture)); f.NoScratch();
    }
    private static void IntegrityAndPartialResults()
    {
        using var f = new Fixture(); File.WriteAllText(f.Main, string.Concat(Enumerable.Repeat("1|1|" + new string('x', 1024) + "\n", 1000)), new UTF8Encoding(false));
        using var capture = f.Host.CaptureHistorySourceAsync(f.Proposal(), f.Main).GetAwaiter().GetResult();
        // Corrupt only the private scratch descriptor; no test or product writes through a source handle.
        var sources = (Array)typeof(HistorySourceCapture).GetField("sources", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(capture)!;
        var source = sources.GetValue(0)!; var image = source.GetType().GetProperty("Image")!.GetValue(source)!;
        var file = (FileStream)image.GetType().GetField("file", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(image)!;
        var handle = file.SafeFileHandle; RandomAccess.Write(handle, [0xff], 65536);
        f.Host.BeginCapturedHistoryImport(capture);
        Reject<InvalidDataException>(() => Drain(f.Host));
        var damage = f.Host.ListEncounters(0, 16).Items.Last().Damage;
        Check(damage > 0 && damage < 1000, "integrity failure lost/over-published partial result");
        Check(handle.IsClosed, "scratch handle survived failed import"); f.NoScratch();
    }
    private static void CancellationAndStop()
    {
        using var f = new Fixture(); File.WriteAllText(f.Main, "1|10\n");
        using var cancel = new CancellationTokenSource(); cancel.Cancel();
        Reject<OperationCanceledException>(() => f.Host.CaptureHistorySourceAsync(f.Proposal(), f.Main, cancellation: cancel.Token).GetAwaiter().GetResult());
        var ready = f.Host.CaptureHistorySourceAsync(f.Proposal(), f.Main).GetAwaiter().GetResult();
        f.Host.StopProcessing(); Check(ready.State == HistorySourceCaptureState.Disposed, "stop retained ready bytes");
        Reject<ObjectDisposedException>(() => f.Host.CaptureHistorySourceAsync(f.Proposal(), f.Main)); f.NoScratch();
    }
    private static void FailedImportShutdown(bool dispose)
    {
        using var f = new Fixture(); File.WriteAllText(f.Main, string.Concat(Enumerable.Repeat("1|1\n", 4000)), new UTF8Encoding(false));
        var live = Path.Combine(f.Directory.FullName, "live.log"); File.WriteAllText(live, "");
        f.Root.LogFilePath = live; f.Root.OpenLog(false, false);
        using var capture = f.Host.CaptureHistorySourceAsync(f.Proposal(), f.Main).GetAwaiter().GetResult();
        f.Root.BeforeLogLineRead += (_, _) => throw new InvalidDataException("synthetic-import-stop");
        f.Host.BeginCapturedHistoryImport(capture);
        var deadline = DateTime.UtcNow.AddSeconds(5);
        while (f.Host.ImportProgress.RecordsDispatched == 0)
        { f.Host.AdvanceImport(false); Check(DateTime.UtcNow < deadline, "no accepted record before shutdown"); Thread.Sleep(1); }
        // Force an independent settings cleanup failure; both errors must survive disposal.
        if (dispose) { f.Root.OriginalLoglineCustomTriggers = true; System.IO.Directory.CreateDirectory(Path.Combine(f.Directory.FullName, "CustomTriggers.xml")); }
        Exception? observed = null;
        try { if (dispose) f.Host.Dispose(); else f.Host.StopProcessing(); } catch (Exception error) { observed = error; }
        Check(observed is not null && observed.ToString().Contains("synthetic-import-stop", StringComparison.Ordinal), "terminal import error was hidden");
        Check(observed is AggregateException, "shutdown stopped before collecting cleanup errors");
        if (dispose)
        {
            Check(ActGlobals.oFormActMain is null && ActGlobals.oFormSpellTimers is null && ActGlobals.oFormEncounterLogs is null, "failed dispose retained ACT globals");
            Check(((AggregateException)observed!).Flatten().InnerExceptions.Count >= 2, "independent settings error was hidden");
        }
        else
        {
            Check((bool)typeof(FormActMain).GetField("actionsStopped", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(f.Root)!, "failed stop left combat runtime running");
            f.Host.Dispose(); Check(ActGlobals.oFormActMain is null, "failed-stop retry retained globals");
        }
        using var exclusive = new FileStream(live, FileMode.Open, FileAccess.ReadWrite, FileShare.None);
        f.NoScratch();
    }

    private sealed class FailingWriter(FileStream target, string mode) : Stream
    {
        public override bool CanWrite => true;
        public override bool CanRead => false;
        public override bool CanSeek => false;
        public override long Length => throw new NotSupportedException();
        public override long Position { get => throw new NotSupportedException(); set => throw new NotSupportedException(); }
        public override async ValueTask WriteAsync(ReadOnlyMemory<byte> buffer, CancellationToken cancellationToken = default)
        {
            await target.WriteAsync(mode.StartsWith("write", StringComparison.Ordinal) ? buffer[..Math.Min(31, buffer.Length)] : buffer, cancellationToken);
            if (mode.StartsWith("write", StringComparison.Ordinal)) throw new IOException("synthetic disk write failure");
        }
        public override Task FlushAsync(CancellationToken cancellationToken) => mode.StartsWith("flush", StringComparison.Ordinal)
            ? Task.FromException(new IOException("synthetic flush failure")) : target.FlushAsync(cancellationToken);
        public override void Flush() => throw new NotSupportedException();
        public override int Read(byte[] buffer, int offset, int count) => throw new NotSupportedException();
        public override void Write(byte[] buffer, int offset, int count) => throw new NotSupportedException();
        public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();
        public override void SetLength(long value) => throw new NotSupportedException();
        protected override void Dispose(bool disposing)
        { if (disposing && mode.EndsWith("cleanup", StringComparison.Ordinal)) throw new IOException("synthetic adapter cleanup failure"); base.Dispose(disposing); }
    }
    private static void WriteAndFlushFailures()
    {
        using var f = new Fixture(); var bytes = Encoding.UTF8.GetBytes("1|10|" + new string('x', 1024) + "\n"); File.WriteAllBytes(f.Main, bytes);
        var type = typeof(FormActMain).Assembly.GetType("Advanced_Combat_Tracker.CapturedLogImage")!;
        var method = type.GetMethod("CopyAsync", BindingFlags.NonPublic | BindingFlags.Static)!;
        foreach (var mode in new[] { "write", "flush", "write-cleanup", "flush-cleanup" })
        {
            using var source = File.OpenRead(f.Main); Microsoft.Win32.SafeHandles.SafeFileHandle? scratch = null;
            Func<FileStream, Stream> adapter = target => { scratch = target.SafeFileHandle; return new FailingWriter(target, mode); };
            var task = (Task)method.Invoke(null, [source, source.Length, f.Directory.FullName, CancellationToken.None, (Action<int>)(_ => { }), adapter])!;
            Exception? error = null; try { task.GetAwaiter().GetResult(); } catch (Exception failure) { error = failure; }
            Check(error is not null && error.ToString().Contains(mode.StartsWith("write", StringComparison.Ordinal) ? "disk write" : "flush", StringComparison.Ordinal), "write/flush error hidden");
            if (mode.EndsWith("cleanup", StringComparison.Ordinal)) Check(error is AggregateException aggregate && aggregate.Flatten().InnerExceptions.Count == 2, "write and cleanup errors not preserved");
            Check(scratch is { IsClosed: true }, "failed image retained target descriptor"); f.NoScratch();
            Check(File.ReadAllBytes(f.Main).SequenceEqual(bytes), "failed scratch write changed original source");
        }
    }
    private static void ExactBudgetBoundaries()
    {
        using var f = new Fixture(); File.WriteAllText(f.Main, "1|10\n"); File.WriteAllText(f.Side("alpha"), "1|20\n"); var snapshot = f.Proposal();
        var exact = new FileInfo(f.Main).Length + new FileInfo(f.Side("alpha")).Length;
        using (var image = f.Host.CaptureHistorySourceAsync(snapshot, f.Main, companions: [new("alpha")], maximumBytes: exact).GetAwaiter().GetResult())
            Check(image.Files.Sum(file => file.Length) == exact, "exact aggregate budget rejected");
        Reject<InvalidDataException>(() => f.Host.CaptureHistorySourceAsync(snapshot, f.Main, companions: [new("alpha")], maximumBytes: exact - 1).GetAwaiter().GetResult());
        Reject<ArgumentOutOfRangeException>(() => f.Host.CaptureHistorySourceAsync(snapshot, f.Main, maximumBytes: 4L * 1024 * 1024 * 1024 + 1));
        if (OperatingSystem.IsLinux())
        {
            // ftruncate creates a synthetic sparse file: validate the 4 GiB bound without writing 4 GiB.
            using (var file = File.OpenWrite(f.Main)) file.SetLength(4L * 1024 * 1024 * 1024 + 1);
            snapshot = f.Proposal(); Reject<InvalidDataException>(() => f.Host.CaptureHistorySourceAsync(snapshot, f.Main).GetAwaiter().GetResult());
            Check(f.Host.HistoryCaptureProgress.BytesCopied == 0, "oversized source was copied before the bound check");
        }
        f.NoScratch();
    }

    private static void ChangeDuringCapture(string mode)
    {
        using var f = new Fixture();
        // A synthetic sparse source bounds runtime while leaving enough chunks to intervene after admission.
        using (var file = File.Create(f.Main)) file.SetLength(64 * 1024 * 1024);
        var initial = new FileInfo(f.Main).Length; var modified = File.GetLastWriteTimeUtc(f.Main);
        using var cancellation = new CancellationTokenSource();
        var task = f.Host.CaptureHistorySourceAsync(f.Proposal(), f.Main, cancellation: cancellation.Token);
        Check(SpinWait.SpinUntil(() => f.Host.HistoryCaptureProgress.BytesCopied > 0 || task.IsCompleted, TimeSpan.FromSeconds(5)), "capture never started");
        Check(!task.IsCompleted, "capture finished before the concurrent boundary could be exercised");
        var old = f.Main + ".old";
        switch (mode)
        {
            case "cancel": cancellation.Cancel(); break;
            case "grow": using (var file = new FileStream(f.Main, FileMode.Open, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete)) { file.Position = file.Length; file.WriteByte(1); } break;
            case "rewrite":
                using (var file = new FileStream(f.Main, FileMode.Open, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete)) file.WriteByte(1);
                File.SetLastWriteTimeUtc(f.Main, modified); break;
            case "replace":
                File.Move(f.Main, old); using (var file = File.Create(f.Main)) file.SetLength(initial);
                File.SetLastWriteTimeUtc(f.Main, modified); break;
            case "history": File.AppendAllText(Path.Combine(f.Directory.FullName, "history/index.xml"), " "); break;
            case "stop": f.Host.StopProcessing(); break;
            default: throw new Exception("unknown capture boundary");
        }
        if (mode is "cancel" or "stop") Reject<OperationCanceledException>(() => task.GetAwaiter().GetResult());
        else if (mode == "history") Reject<InvalidOperationException>(() => task.GetAwaiter().GetResult());
        else Reject<IOException>(() => task.GetAwaiter().GetResult());
        if (mode != "stop") Check(f.Host.HistoryCaptureProgress.BytesCopied <= initial, "growing source escaped its captured length");
        Check(f.Host.ListEncounters(0, 16).Total == 1, "failed preparation published partial graph");
        using var exclusive = new FileStream(mode == "replace" ? old : f.Main, FileMode.Open, FileAccess.ReadWrite, FileShare.None);
        f.NoScratch();
    }
    private static void ActiveImportStopAndCallbackGuard()
    {
        using var f = new Fixture(); File.WriteAllText(f.Main, string.Concat(Enumerable.Repeat("1|1\n", 4000)), new UTF8Encoding(false));
        using var image = f.Host.CaptureHistorySourceAsync(f.Proposal(), f.Main).GetAwaiter().GetResult();
        var rejected = false;
        LogLineEventDelegate callback = (_, _) =>
        {
            try { f.Host.Dispose(); } catch (InvalidOperationException) { rejected = true; }
            Check(ReferenceEquals(ActGlobals.oFormActMain, f.Root), "reentrant dispose altered globals");
        };
        f.Root.OnLogLineRead += callback;
        f.Host.BeginCapturedHistoryImport(image);
        var deadline = DateTime.UtcNow.AddSeconds(5);
        while (f.Host.ImportProgress.RecordsDispatched == 0)
        { f.Host.AdvanceImport(false); Check(DateTime.UtcNow < deadline, "no callback"); Thread.Sleep(1); }
        Check(rejected, "unsafe callback disposal was accepted");
        f.Root.OnLogLineRead -= callback;
        f.Host.StopProcessing();
        Check(f.Host.ListEncounters(0, 16).Items.Last().Damage > 0, "normal active-stop lost accepted partial graph");
        Check((bool)typeof(FormActMain).GetField("actionsStopped", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(f.Root)!, "active-stop did not stop runtime");
        f.NoScratch();
    }
    private static void ProducerFailureDuringStop()
    {
        using var f = new Fixture(); File.WriteAllText(f.Main, string.Concat(Enumerable.Repeat("1|1\n", 4000)), new UTF8Encoding(false));
        using var image = f.Host.CaptureHistorySourceAsync(f.Proposal(), f.Main).GetAwaiter().GetResult();
        f.Host.BeginCapturedHistoryImport(image); f.Host.AdvanceImport(false);
        // Inject the same channel terminal error that the actual input task reports on read/integrity failure.
        var operation = typeof(FormActMain).GetField("activeImport", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(f.Root)!;
        var input = (System.Threading.Channels.Channel<string[]>)operation.GetType().GetField("input", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(operation)!;
        Check(input.Writer.TryComplete(new IOException("synthetic-producer-io")), "producer completed before error injection");
        Exception? failure = null; try { f.Host.Dispose(); } catch (Exception error) { failure = error; }
        Check(failure is AggregateException && failure.ToString().Contains("synthetic-producer-io", StringComparison.Ordinal), "cancel hid the recorded producer failure");
        Check(ActGlobals.oFormActMain is null && ActGlobals.oFormSpellTimers is null && ActGlobals.oFormEncounterLogs is null, "producer failure stopped final disposal");
        f.NoScratch();
    }

    private static void RangeCutoffIgnoresUndispatchedSuffix()
    {
        using var f = new Fixture(); File.WriteAllBytes(f.Main, [..Encoding.UTF8.GetBytes("1|10\n3|20\n4|999\n"), 0xff, 10]);
        using var capture = f.Host.CaptureHistorySourceAsync(f.Proposal(), f.Main).GetAwaiter().GetResult();
        f.Host.BeginCapturedHistoryImport(capture); var report = Drain(f.Host);
        Check(report.Damage == 30 && report.RecordsDispatched == 2, "unconsumed decoder suffix escaped normal range cutoff");
        f.NoScratch();
    }

}
