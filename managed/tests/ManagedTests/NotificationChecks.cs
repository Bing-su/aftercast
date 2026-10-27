using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Bridge;
using System.Windows.Forms;

internal static class NotificationChecks
{
    private static void Check(bool value, string message) { if (!value) throw new InvalidOperationException(message); }
    private static void Reject(Action action)
    {
        try { action(); }
        catch (Exception error) when (error is InvalidOperationException or ArgumentOutOfRangeException) { return; }
        throw new Exception("Expected notification operation rejection");
    }
    private sealed class Target : Control
    {
        internal int Calls;
        internal void Show(object? sender, EventArgs args) => Calls++;
    }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-notifications-");
        try
        {
            FormActMain previous;
            using (var context = new OwnerContext(_ => throw new Exception("Unexpected owner error")))
            using (var host = context.Run(() => new HostEnvironment(directory.FullName)))
            {
                var root = previous = ActGlobals.oFormActMain!;
                Callbacks(root, context);
                Check(host.DiagnosticsSnapshot.Item2.Any(line => line.Contains("Notification Show:") && line.Contains("synthetic Show failure")),
                    "Show exception was not recorded on the original host");
                InfoLogFailure(root, host);
                Retention(root);
            }
            using (var host = new HostEnvironment(directory.FullName))
            {
                Check(ActGlobals.oFormActMain!.ReadNotifications(0,16).Items.Length == 0, "Notification callbacks leaked across sessions");
                Reject(() => previous.ReadNotifications(0,16));
                Reject(() => previous.ShowNotification(1));
                Reject(() => previous.NotificationAdd("late", "closed"));
            }
            Console.WriteLine("Notification owner dispatch, sender, repeat, disposed/throwing targets, retention and session checks passed.");
        }
        finally { directory.Delete(true); }
    }
    private static void Callbacks(FormActMain root, OwnerContext context)
    {
        var owner = Environment.CurrentManagedThreadId;
        var sender = new object(); var calls = 0;
        EventArgs? previousArgs = null;
        root.NotificationAdd("show", "message", (actual, args) =>
        {
            Check(ReferenceEquals(actual,sender) && Environment.CurrentManagedThreadId == owner,
                "Show changed sender or owner thread");
            Check(SynchronizationContext.Current == context && !ReferenceEquals(previousArgs,args), "Show lost owner context or fresh EventArgs");
            previousArgs = args; calls++;
            root.NotificationAdd("nested", "callback can add history");
        }, sender);
        var id = root.ReadNotifications(0,16).Items.Single().Id;
        Task.Run(() => Reject(() => root.ShowNotification(id))).GetAwaiter().GetResult();
        Check(calls == 0 && !root.ReadNotifications(0,16).Items[0].Read, "Off-owner Show invoked or marked read");
        for (var index = 0; index < 2; index++)
        {
            var shown = context.Run(() => root.ShowNotification(id));
            Check(shown.Read && shown.CanShow && shown.ShowError is null, "Successful Show was disabled");
        }
        Check(calls == 2, "Intentional repeat Show did not invoke again");
        using var target = new Target();
        root.NotificationAdd("disposed", "target", target.Show);
        var disposedId = root.ReadNotifications(0,16).Items.Last().Id;
        target.Dispose();
        Check(!root.ShowNotification(disposedId).CanShow && target.Calls == 0, "Disposed target invoked");
        var failures = 0;
        root.NotificationAdd("throw", "once", (_, _) => { failures++; throw new InvalidOperationException("synthetic Show failure"); });
        var failingId = root.ReadNotifications(0,16).Items.Last().Id;
        var failed = root.ShowNotification(failingId);
        Check(!failed.CanShow && failed.Read && failed.ShowError is not null, "Throwing callback remained enabled");
        _ = root.ShowNotification(failingId);
        Check(failures == 1 && root.ReadNotifications(0,16).Items.Last().ShowError is not null, "Failed Show retried implicitly");
        Check(root.ReadNotifications(0,16).Items.Count(item => !item.CanShow) >= 4, "No-handler entries incorrectly enabled");
        Reject(() => root.ShowNotification(99999));
        Reject(() => root.ReadNotifications(0,0));
        Reject(() => root.ReadNotifications(0,17));
    }
    private sealed class BrokenWriter : StringWriter
    {
        public override void WriteLine(string? value) => throw new IOException("synthetic notification log failure");
    }
    private static void InfoLogFailure(FormActMain root, HostEnvironment host)
    {
        var output = Console.Out;
        using var broken = new BrokenWriter();
        try
        {
            Console.SetOut(broken);
            root.NotificationAdd("retained", "despite information log failure");
        }
        finally { Console.SetOut(output); }
        Check(root.ReadNotifications(0,16).Items.Any(item => item.Title == "retained"), "Info log failure discarded the notification");
        Check(host.DiagnosticsSnapshot.Item2.Any(line => line.Contains("Notification info log:") && line.Contains("synthetic notification log failure")),
            "Info log failure was not reported to host diagnostics");
    }
    private static void Retention(FormActMain root)
    {
        var first = root.ReadNotifications(0,16).Items[0].Id;
        Parallel.For(0,300,index => root.NotificationAdd("producer", index.ToString()));
        var page = root.ReadNotifications(0,16);
        Check(page.Omitted > 0 && page.Items.Length == 16, "Bounded producer history lost omission/paging status");
        Reject(() => root.ShowNotification(first));
        var count = 0; ulong cursor = 0;
        do
        {
            page = root.ReadNotifications(cursor,16);
            foreach (var item in page.Items) { Check(item.Id > cursor, "IDs are not ordered"); cursor = item.Id; count++; }
        } while (page.Items.Length != 0);
        Check(count == 256, "Notification count cap is not enforced");
        var before = page;
        root.NotificationAdd(new string('x',32769), "oversized");
        page = root.ReadNotifications(cursor,16);
        Check(page.Items.Length == 0 && page.Omitted == before.Omitted + 1 && page.LatestId == before.LatestId + 1,
            "Oversized record lost monotonic identity or omission status");
        CharacterBudget(root);
    }
    private static void CharacterBudget(FormActMain root)
    {
        var output = Console.Out;
        try
        {
            Console.SetOut(TextWriter.Null);
            for (var index = 0; index < 40; index++) root.NotificationAdd(new string('x',32768), "");
        }
        finally { Console.SetOut(output); }
        ulong cursor = 0; var characters = 0; var count = 0;
        NotificationPage page;
        do
        {
            page = root.ReadNotifications(cursor,16);
            foreach (var item in page.Items)
            { cursor = item.Id; characters += item.Title.Length + item.Message.Length; count++; }
        } while (page.Items.Length != 0);
        Check(count == 32 && characters == 1024 * 1024, "Text budget failed to evict oldest records independently of count cap");
    }

}
