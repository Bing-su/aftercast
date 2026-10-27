# ACT notification history and Show callbacks

The reference is ACT 3.8.5.288 `FormActMain.NotificationAdd` and
`NotificationHistoryItem.btnShow_Click`. Notifications are distinct from host
diagnostics: an entry may retain a plugin callback and its original sender object.

## Implemented managed contract

| Operation                | Behavior                                                                                                                                                                                                                                                                      |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `NotificationAdd`        | Accepts title/message and an optional callback/sender. Producer threads only append bounded model state; no native controls are accessed. Null text becomes empty text.                                                                                                       |
| History                  | Increasing IDs, source wall-clock DateTime ticks, title/message, read state, Show availability and a failure summary. Pages contain at most 16 entries.                                                                                                                       |
| Explicit Show            | Requires the root's creating thread and the Bridge owner context when invoked through the host. Marks the entry read and invokes the original delegate with its original sender and a fresh `EventArgs`. Successful actions remain available for intentional repeated clicks. |
| Disposed callback target | If `delegate.Target` is a disposed WinForms `Control`, mark read, release the callback/sender and disable Show without invoking it. This matches ACT's target check; closure-captured objects are not recursively inspected.                                                  |
| Callback exception       | Log the exception to the original root's host diagnostics under `Notification Show`, retain a bounded failure summary and release/disable the action. The exception does not escape the Show method.                                                                          |
| Missing/expired ID       | Reject before invoking plugin code. Each root owns its own IDs and delegates; a disposed root rejects reads, additions and Show.                                                                                                                                              |
| Bounds                   | Retain at most 256 entries and 1 Mi UTF-16 text characters. An entry larger than 32,768 title/message characters is omitted. Eviction and oversized entries advance the cumulative omission count; IDs never reset within a root.                                             |
| Shutdown                 | Release retained callbacks and sender objects before stopping workers and disposing controls. In-flight callbacks are owned by the host owner thread.                                                                                                                         |

Information-log failures are reported to the original host diagnostics without
propagating an exception after the entry has already been added.

Unlike ACT's native history control, ingestion does not marshal model-only work to
the UI thread. The history lock serializes producer threads, while callback execution
occurs outside that lock so a callback may add another notification safely. The
bounded retention policy is an aftercast resource limit, not a claim about ACT's
unbounded history behavior.

## Rust transport and GUI

`HostHandle::notifications` returns owned records and an opaque session-owned cursor.
`show_notification` accepts an opaque notification ID and invokes the callback through
`PluginSession.OnOwner`. Old-session IDs/cursors are rejected before dispatch. The
fixed ABI reply capacity and command fields are validated before invoking any callback.
Delegates and sender objects never cross the process boundary.

The Notifications tab uses the existing Rust/UI Button, Badge and Accordion components.
It displays receipt time, text, per-entry read state, Show availability/failure and the
cumulative omission count. First-page refresh and previous/next paging retain owned
records. A full final page can lead to an empty next page; Previous remains available.
Refresh is explicit; this pane does not poll or claim a global unread count.

The controller admits one request at a time. An explicit Show blocks other host mutations
until completion but remains independent of pane lifetime. Navigating away does not cancel
or repeat the action, and completion does not change the selected screen. Stop/quit
invalidates the UI session and clears history; replies from older sessions cannot update
new state or clear a newer request. A successful Show remains available for a later
intentional click. An acknowledged Show is never automatically retried after an error.

Mark-all, direct mark-read without Show, clear-read and badge-blinking controls remain
subsequent ACT history operations. Native callback effects remain platform-dependent;
the partial WinForms rendering bridge is still planning only.

Synthetic managed tests cover owner identity, original sender/fresh event arguments,
repeat clicks, reentrant additions, absent handlers, disposed targets, callback
exceptions, concurrent producers, paging/retention, expired IDs and session cleanup.
Actual native callback window display, focus and input remain Windows validation;
Linux tests do not establish WinForms rendering compatibility. The partial WinForms
rendering bridge remains a separate plan, not an implemented notification feature.
