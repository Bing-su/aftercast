using System.Windows.Forms;

namespace Aftercast.ActCompat
{
    public sealed record NotificationItem(ulong Id, long TimeTicks, string Title, string Message, bool Read,
        bool CanShow, string? ShowError);
    public sealed record NotificationPage(ulong LatestId, ulong Omitted, NotificationItem[] Items, int Retained, int Unread);
    public sealed record NotificationStatus(ulong LatestId, ulong Omitted, int Retained, int Unread);
    public sealed record NotificationChange(int Changed, NotificationPage Page);
}

namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;

    public partial class FormActMain
    {
        private sealed class Notification(ulong id, string title, string message, EventHandler? callback, object? sender)
        {
            internal readonly ulong Id = id;
            internal readonly long TimeTicks = DateTime.Now.Ticks;
            internal readonly string Title = title, Message = message;
            internal EventHandler? Callback = callback;
            internal object? Sender = sender;
            internal bool Read;
            internal string? Error;
            internal NotificationItem Snapshot() => new(Id, TimeTicks, Title, Message, Read, Callback is not null, Error);
            internal void Disable() { Callback = null; Sender = null; }
        }
        private readonly object notificationGate = new();
        private readonly LinkedList<Notification> notifications = new();
        private ulong notificationSequence, omittedNotifications;
        private int notificationCharacters;
        private bool notificationsClosed;

        public void NotificationAdd(string title, string message, EventHandler? showCallback = null, object? senderObject = null)
        {
            title ??= ""; message ??= "";
            lock (notificationGate)
            {
                ObjectDisposedException.ThrowIf(notificationsClosed || IsDisposed, this);
                var id = checked(++notificationSequence);
                // Bound plugin-owned text/delegates; skipped and evicted records remain visible in the omission count.
                if ((long)title.Length + message.Length > 32768) { omittedNotifications++; return; }
                while (notifications.Count >= 256 || notificationCharacters + title.Length + message.Length > 1024 * 1024)
                {
                    var old = notifications.First!.Value;
                    notificationCharacters -= old.Title.Length + old.Message.Length;
                    old.Disable(); notifications.RemoveFirst(); omittedNotifications++;
                }
                notifications.AddLast(new Notification(id, title, message, showCallback, senderObject));
                notificationCharacters += title.Length + message.Length;
            }
            // No native controls are touched on producer threads. Show is dispatched explicitly by the owner.
            try { WriteInfoLog("Notification" + Environment.NewLine + title + Environment.NewLine + message); }
            catch (Exception error) { WriteExceptionLog(error, "Notification info log"); }
        }

        public NotificationPage ReadNotifications(ulong after, int limit)
        {
            if (limit is < 1 or > 16) throw new ArgumentOutOfRangeException(nameof(limit));
            lock (notificationGate)
            {
                ObjectDisposedException.ThrowIf(notificationsClosed || IsDisposed, this);
                if (after > notificationSequence) throw new ArgumentOutOfRangeException(nameof(after));
                return NotificationPageSnapshot(after, limit);
            }
        }

        // Snapshot bounds refer to the whole retained history, never just the visible page.
        private NotificationPage NotificationPageSnapshot(ulong after, int limit) => new(notificationSequence, omittedNotifications,
            notifications.Where(item => item.Id > after).Take(limit).Select(item => item.Snapshot()).ToArray(),
            notifications.Count, notifications.Count(item => !item.Read));

        public NotificationStatus ReadNotificationStatus()
        {
            lock (notificationGate)
            {
                ObjectDisposedException.ThrowIf(notificationsClosed || IsDisposed, this);
                return new(notificationSequence, omittedNotifications, notifications.Count, notifications.Count(item => !item.Read));
            }
        }

        public NotificationChange MarkNotificationsRead(ulong through) => ChangeNotifications(through, false);
        public NotificationChange ClearReadNotifications(ulong through) => ChangeNotifications(through, true);
        private NotificationChange ChangeNotifications(ulong through, bool clearRead)
        {
            if (Environment.CurrentManagedThreadId != ownerThread)
                throw new InvalidOperationException("Notification changes require the host owner thread");
            lock (notificationGate)
            {
                ObjectDisposedException.ThrowIf(notificationsClosed || IsDisposed, this);
                if (through > notificationSequence) throw new ArgumentOutOfRangeException(nameof(through));
                var changed = 0;
                for (var node = notifications.First; node is not null;)
                {
                    var next = node.Next;
                    var item = node.Value;
                    if (item.Id <= through && (clearRead ? item.Read : !item.Read))
                    {
                        changed++;
                        if (clearRead)
                        {
                            notificationCharacters -= item.Title.Length + item.Message.Length;
                            item.Disable(); notifications.Remove(node);
                        }
                        else item.Read = true;
                    }
                    node = next;
                }
                // Publish the first page/counts under the same lock; additions after this boundary remain unread.
                return new(changed, NotificationPageSnapshot(0, 16));
            }
        }

        public NotificationItem ShowNotification(ulong id)
        {
            if (Environment.CurrentManagedThreadId != ownerThread)
                throw new InvalidOperationException("Notification Show requires the host owner thread");
            Notification item;
            EventHandler? callback;
            object? sender;
            lock (notificationGate)
            {
                ObjectDisposedException.ThrowIf(notificationsClosed || IsDisposed, this);
                item = notifications.FirstOrDefault(item => item.Id == id)
                    ?? throw new InvalidOperationException("Notification no longer exists");
                item.Read = true; callback = item.Callback; sender = item.Sender;
                if (callback?.Target is Control control && control.IsDisposed) { item.Disable(); callback = null; }
            }
            // A callback can add another notification. Never hold the history lock across plugin code.
            try { callback?.Invoke(sender, new EventArgs()); }
            catch (Exception error)
            {
                lock (notificationGate) { item.Error = "Notification Show failed; see host diagnostics"; item.Disable(); }
                WriteExceptionLog(error, "Notification Show");
            }
            lock (notificationGate) return item.Snapshot();
        }

        private void CloseNotifications()
        {
            lock (notificationGate)
            {
                notificationsClosed = true;
                foreach (var item in notifications) item.Disable();
                notifications.Clear(); notificationCharacters = 0;
            }
        }
    }
}
