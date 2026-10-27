# ACT notification history and controls

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
List refresh is explicit. An App-owned one-second model-only status poll updates
the whole retained-history unread badge even while another tab is open. It runs
only when the Host is ready and other reads/actions do not own admission. Zero
unread hides the badge, 1–9 shows the number, and 10+ shows the capped indicator
with the exact count in its title. An unavailable count is shown as unknown,
not zero; errors remain visible in Notifications. Blinking defaults to true,
respects reduced motion, and can be disabled without hiding the count. The draft
previews immediately; explicit Save settings persists `ui.notification_blink` using
the existing atomic annotated-TOML path. Save failure preserves the draft and the
last acknowledged ConfigFile. Accepted disk writes survive navigation and Stop.

The controller owns one current request token; a user action may supersede a model-only status read and discard its late reply. An explicit Show blocks other host mutations
until completion but remains independent of pane lifetime. Navigating away does not cancel
or repeat the action, and completion does not change the selected screen. Stop/quit
invalidates the UI session and clears history; replies from older sessions cannot update
new state or clear a newer request. A successful Show remains available for a later
intentional click. An acknowledged Show is never automatically retried after an error.

Mark-all and clear-read controls use the managed/Bridge/Rust APIs described below.
Buttons bind the rendered page's whole-history upper bound, not a later poll's
count snapshot. The scope help explains that other pages are included and newer
arrivals are preserved. Success acknowledges the returned first page and resets
pagination; errors retain the old page for explicit refresh without automatic retry. Native callback effects remain platform-dependent;
the partial WinForms rendering bridge is still planning only.

Synthetic managed tests cover owner identity, original sender/fresh event arguments,
repeat clicks, reentrant additions, absent handlers, disposed targets, callback
exceptions, concurrent producers, paging/retention, expired IDs and session cleanup.
Actual native callback window display, focus and input remain Windows validation;
Linux tests do not establish WinForms rendering compatibility. The partial WinForms
rendering bridge remains a separate plan, not an implemented notification feature.

## Whole-history bulk API

The pinned ACT reference `FormActMain.btnNotificationsMarkRead_Click` sets Read on
every retained history control. `btnNotificationsClearRead_Click` disposes only
read controls. `NotificationCountDraw` counts all unread entries: no badge for zero,
1–9 individually, a 10-or-more indicator otherwise; `cbNotificationBlink` defaults
to true and is persisted through ACT's settings serializer. Turning blinking off
does not hide the count. These reference methods were inspected from the pinned
local binary; native timer animation/input is not exercised by Linux contracts.

For asynchronous requests, the accepted `NotificationStatus.snapshot_cursor` bounds
the whole observed history. It differs from a page's `next_cursor`: a one-item page
can carry the full history upper bound. Explicitly mark/clear through that bound;
newer arrivals remain untouched, even if read before a delayed clear executes.
This is an explicit asynchronous boundary rather than an unbounded changing set.

| Operation                           | Contract                                                                                                                                                                                                                                               |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `notification_status()`             | Whole retained/unread counts, omission count and a session-owned snapshot bound. No plugin callback.                                                                                                                                                   |
| `mark_notifications_read(through)`  | Mark retained entries through the bound read. Do not invoke or disable Show callbacks. Already-read or expired entries are no-ops.                                                                                                                     |
| `clear_read_notifications(through)` | Remove only read entries through the bound; release callback/sender references. Clear does not increase omissions or reset IDs. Cleared Show IDs are rejected permanently.                                                                             |
| Result                              | Changed count plus first page and whole counts, captured under the mutation lock. Reset pagination to this acknowledged first page rather than editing stale pages locally.                                                                            |
| Repetition/failure                  | Repeating the same bound without intervening read-state changes is idempotent. Future bounds, disposed roots, wrong owner thread and foreign sessions are rejected. Validation precedes mutation; no callback/logging is invoked during bulk mutation. |
| Retention                           | The existing 256-entry/text bound still applies. Explicit clear frees text capacity; later IDs remain monotonic. No global total of all historical arrivals is implied by retained/unread counts.                                                      |

Bridge commands 3/4 mutate and command 5 reads status. Fixed reply capacity and
command fields are checked before dispatch; owner-thread isolation and existing
operation admission apply. Rust rejects impossible counts/page boundaries and
foreign-session bounds. No new cancellation or callback preemption is introduced.

Synthetic managed and actual worker regressions cover whole-history vs page scope,
new arrivals between observation and mutation, repeated calls, errors before
mutation, preserved Show callbacks, cleared/old-session IDs, empty histories,
counts and monotonic IDs. Actual synthetic Host controller regressions cover duplicate admissions, stale
same-session and old-session replies, late status vs bulk acknowledgement, screen
navigation, Stop, clear pagination reset, expired Show IDs, overflow vs explicit
clear, callback-generated counts, and failed settings writes/retry/config reload.
A headless Signal test runs the polling future across a Quit tick and cancellation;
it does not inject an actual shutdown failure. Request tokens include session and
serial; a user action supersedes a background count read and rejects its late reply.
Polling skips pending Quit without ending permanently, since a failed shutdown can
keep the same App alive. The Windows desktop scenario covers badge/bulk/blink and
restart, but is only prepared/discovered on Linux; actual screen/input is unverified.
