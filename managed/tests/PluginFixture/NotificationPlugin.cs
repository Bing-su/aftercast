using Advanced_Combat_Tracker;
using System.Windows.Forms;

namespace Aftercast.PluginFixture;

// Callbacks mutate only synthetic notification history; no files, settings or external services.
public sealed class NotificationPlugin : IActPluginV1
{
    private sealed class DisposedTarget : Control
    {
        internal void Show(object? sender, EventArgs args) => throw new Exception("Disposed target must not run");
    }
    public void InitPlugin(TabPage tab, Label status)
    {
        var root = ActGlobals.oFormActMain!;
        var owner = Environment.CurrentManagedThreadId;
        var context = SynchronizationContext.Current;
        var sender = new object(); var calls = 0;
        EventArgs? previous = null;
        root.NotificationAdd("repeat", "synthetic callback", (actual, args) =>
        {
            if (!ReferenceEquals(sender,actual) || Environment.CurrentManagedThreadId != owner ||
                !ReferenceEquals(context,SynchronizationContext.Current) || ReferenceEquals(previous,args))
                throw new Exception("Notification callback ownership changed");
            previous = args;
            root.NotificationAdd("shown", (++calls).ToString());
        }, sender);
        using var target = new DisposedTarget();
        root.NotificationAdd("disposed", "synthetic target", target.Show);
        root.NotificationAdd("throws", "synthetic failure", (_, _) =>
        {
            root.NotificationAdd("failure-attempt", "once");
            throw new InvalidOperationException("synthetic notification callback failure");
        });
        root.NotificationAdd("no action", "synthetic message");
        root.NotificationAdd("fill", "synthetic retention", (_, _) =>
        {
            for (var index = 0; index < 300; index++) root.NotificationAdd("retention", index.ToString());
        });
    }
    public void DeInitPlugin() { }
}
