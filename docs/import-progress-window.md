# Raw import progress window

## ACT reference and desktop mapping

The pinned ACT 3.8.5.288 artifact SHA-256 is
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
`OriginalReference import-policy` inspects actual IL; no original native form or
import thread is run for this probe.

| Original location                         | Verified behavior                                                                                                      |
| ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| FormImportProgress.btnClose_Click IL_0001 | Calls CloseThis.                                                                                                       |
| CloseThis IL_0007–IL_0045                 | Checks importThreadAlive; when active, asks an abort question. Only Yes calls CancelImport; No returns without hiding. |
| CloseThis IL_0046–IL_0078                 | Hides the form, resets displayed batch/progress and stops its operation stopwatch.                                     |
| EndOperationStats                         | Sets operationComplete, stops timing and updates the progress display.                                                 |

Aftercast follows ACT's close decision: while an import is pending, **Close**
and native X request an abort confirmation. **No** keeps both the window and job
running. **Yes** requests cancellation of that confirmed job and hides the window;
the main owner continues draining and retains accepted partial encounters. When
no job is active, Close hides immediately without cancellation. The existing
explicit Cancel import action also remains available without a close request.

The confirmation uses existing Rust/UI controls inside the progress window,
rather than a native WinForms MessageBox. Its owner token prevents an old Yes
from canceling a newer import; a completed confirmed job may be hidden without
an unnecessary cancel. The original native dialog's modality, focus and timing
are not claimed identical. Dioxus 0.7.10 offers only hide or close for native X:
its default hide occurs after event handlers, so aftercast restores the window
once on the next 100 ms observation tick when a question is required. Native X
can therefore briefly hide/resurface; this is a current desktop API limitation,
not permission to omit ACT's confirmation, and requires Windows/native QA.
Root exit explicitly switches the child to WindowCloses before destruction, so
a hidden progress window cannot keep the application alive after shutdown.

## Application-owned projection

| Path            | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Admission       | The common wait for an already accepted ImportJob creates a new session/serial token and copies source kind plus requested character policy. Whole-file, range, supplied Unicode text and captured-history replay use it. File picker failures, candidate discovery/byte capture and ACT XML/gzip archive operations are not ImportJobs and are not presented as new accepted raw imports.                                                                                                    |
| Window          | Automatically request one nonmodal Dioxus window at accepted-job wait when a DesktopContext is available. The top-bar Progress window button opens it explicitly, including before the first import or after completion. A pending/open window is reused; repeated clicks focus the existing window rather than creating additional active windows. Normal close hides the same window, so reopening reuses its existing VirtualDom; only actual destruction releases the singleton lifetime. |
| Ownership       | The main app owns the job, cancellation polling and drain. Windows share bounded plain data through an Arc/Mutex hub, never a Dioxus Signal across VirtualDoms. Opening failure affects the window only and is reported in the main window. Synthetic headless controllers do not pretend to create a native window.                                                                                                                                                                          |
| New requests    | The single progress window follows the latest accepted job. Every cancel/confirmation callback carries the rendered session/serial token, so an old button cannot cancel a later import. Old publish/finish responses cannot overwrite a replacement job. Only the latest snapshot is retained.                                                                                                                                                                                               |
| Completion      | Running/cancellation-requested, success, cancellation, failure and Host-retired states are distinct. The successful ImportReport supplies final record/encounter totals; failure/cancellation preserve the latest observed counters and accepted partial encounters. A cancellation request can legitimately race with successful completion; the actual terminal result wins.                                                                                                                |
| Counters        | Input bytes are the latest observed ImportProgress, not durability or accepted-action acknowledgements. Read-ahead can lead parsing and a fast completion can leave byte counters behind the final record totals. The bar is shown only while pending. Supplied text uses UTF-8-equivalent byte counts. Known excluded companion heads are not an estimate of an unread suffix.                                                                                                               |
| Context/privacy | Show file/range/supplied-text/captured-history kind and the admitted requested character policy; saved captures keep their saved exact identity/range. No clipboard text, source rows, parser exception strings or file contents are copied into the progress snapshot. Failures use a fixed localized message; available main-pane diagnostics retain their existing policy.                                                                                                                 |
| Stop/exit       | Stop retires the current snapshot and its buttons. New Host jobs get a new token; late old completion is ignored. Main-app exit/drop closes the shared hub, rejects new windows/jobs and causes any already queued/mounted progress window to close. Existing owner cancellation/shutdown still drains the job. Window creation that resolves after root closure is closed, not rebound.                                                                                                      |
| Appearance      | Reuse existing Rust/UI-derived Button/Progress and existing Dioxus nonmodal-window construction with Tailwind/Catppuccin and Korean/English copy. Theme/language are copied when the progress window is created, as in the existing log window. No UI library, JavaScript bridge or WinForms rendering implementation is added.                                                                                                                                                               |

ACT CloseThis resets its form controls. Aftercast retains the latest job-owned
snapshot when hidden instead of writing fabricated zeros to shared ImportJob
telemetry: the main pane and cancellation/drain still consume that same owned
operation. Reopen explicitly shows the last admitted job/result, and the next
admission replaces it. This state-ownership mapping does not claim ACT's exact
control-reset appearance or operation/batch stopwatch display.

Progress is an ImportJob projection. Captured-source proposal release or unrelated
post-import UI refresh can still report errors in the main workflow after that
job's own input/action/live-state cleanup has completed. This window does not
claim to replace those workflows' diagnostics or add throughput/batch/ETA metrics
that the current API cannot substantiate.

## Evidence and limits

- Plain-model tests cover terminal states, cancellation/completion races, exact
  owner tokens, session replacement, stale actions/results, bounded retained
  counters, error-text exclusion, duplicate window reservation, close/reopen,
  failed creation/retry and late launch responses.
- Actual isolated Host/controller tests use only temporary synthetic files and
  callback gates. They cover ACT close-No/reopen without cancellation and close-Yes cancellation, visible headless
  opening failure, explicit cancellation/drain with partial encounters, retry,
  failure redaction and root exit/session retirement with late response rejection.
  Existing full/range/supplied-text/capture controllers exercise the shared owner
  wait in the normal aggregate suite.
- The Windows WebView2 suite includes singleton open, close No/Yes confirmation,
  hide/reopen, cancel, next-job completion and main quit with a child window. Linux
  collects/parses this scenario only; native creation, focus, X/keyboard input,
  multi-window rendering and Windows event-loop timing are unverified.
- ser7 startup smoke does not replace native interaction QA. The existing missing
  accessibility bus/apps limitation is unchanged; no system settings or alternate
  machine is used. L6b OS clipboard provider remains approval-blocked; no rejected
  operation is retried and no real clipboard, game or user log is accessed.
