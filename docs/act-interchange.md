# ACT XML/gzip interchange

Public ACT interchange is separate from raw-log parsing and the precise private
history database. Existing readers and writers remain the implementation; no new
XML dialect or report exporter is introduced.

| Contract        | Behavior                                                                                                                                                                                                                                                                                                                                                                                                                        |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Import          | Auto-detect XML versus gzip bytes, initialize parser categories first, end live combat; decode completely before attaching encounters. Malformed input adds no encounters. File and expanded XML remain bounded to 64 MiB. Parser hooks/audio are not replayed. An independent, default-off option matches included log lines for tabbed custom-trigger results after full decoding; it does not run audio, TTS or live timers. |
| Export          | One session-owned retained encounter, explicit XML or gzip format, a new destination only. Atomic publication refuses an existing file, including a destination created during the write.                                                                                                                                                                                                                                       |
| Log policy      | `ActExportOptions.include_logs` controls retained LogLine output; legacy `export_act` and default options include logs. Omitting logs does not strip actions or mutate the original encounter.                                                                                                                                                                                                                                  |
| Private history | Always includes its precise log/identity metadata. Public export options do not alter checkpoint behavior.                                                                                                                                                                                                                                                                                                                      |
| Accepted work   | Existing Archive Begin/Advance and Host shutdown drain apply. Dropping a response future or changing screen does not undo an accepted import/export. File selection/confirmation may be canceled before admission.                                                                                                                                                                                                              |
| Errors          | Missing parent, invalid XML/gzip, unsupported input and destination conflicts are visible errors. Existing destinations remain unchanged; no replace/overwrite permission is inferred.                                                                                                                                                                                                                                          |

Bridge export flags use bit 0 for gzip and bit 1 for **omit** logs. Zero/one retain
the prior include-logs behavior. Import bit 0 requests trigger results; other import flags and all flags on history commands are rejected before admission.

Synthetic isolated-worker regressions exercise XML/gzip × included/omitted logs,
source graph preservation, refusal to overwrite and exact destination bytes,
missing parent failure and private-history restart restoration. Existing ACT
archive/original comparisons remain required. Native file dialogs/clipboard and
Windows input/focus are separate execution checks.

## Application workflow

Import/Export now contains an ACT interchange panel built with the existing local
Rust/UI Button/Input/Label/Accordion adapters. Import accepts an explicit path or
native file selection and a deliberate Import action. Export captures the selected
encounter, destination, format and include-log policy for a separate confirmation.
The format is not inferred from the filename. Drafts survive failure and Stop for
the application session; they are not new persisted Options preferences.

Picker cancellation and confirmation cancellation perform no archive operation.
Every rendered action uses its session/generation token. Selection changes reject
old export confirmations, and navigation/Stop/Quit invalidate picker reservations
before a native dialog is created and before a late response is applied. Accepted
I/O completes under application ownership after navigation and Host shutdown drains
it; it has no misleading in-flight cancel/rollback button. Existing-file conflicts
are visible, even when a native save dialog offered replacement.

Four actual isolated-Host GUI-controller scenarios cover XML/gzip × log policy,
empty/malformed XML/gzip, source graph preservation, absent selection, missing parent,
a destination directory write failure with temporary-file cleanup, existing-file
and post-confirmation conflict preservation, picker/confirmation cancel and stale
responses, immutable admitted options, navigation and shutdown drain. Two pure UI
state tests cover path/draft validation and pre-dialog lifecycle/render-token guards.
A Windows WebView2 scenario is defined for selection/confirmation/cancel/import/
non-overwrite, but is not executed on ser7. Native picker/focus/keyboard interaction
and Windows execution remain unverified; Linux app startup is not click QA.

This closes the Linux implementation of **L7a** only. Raw-log options L6a–d,
text/HTML/table-XML reports L7b and real/synthetic ODBC work L7c remain separate.

## Archive trigger matching

ACT `XmlToEncounters` checks `cbParseCt` on each archive LogLine independently
of raw-log `cbImportCustomTriggers`. Aftercast decodes the whole archive before
matching, preserving malformed-input atomicity. It applies the existing original
text option, category filter and .NET regex engine to retained archive logs and
records tabbed import results without audio, speech, timers or parser hooks.
The settings default is saved separately and takes effect on the next application
launch; the per-request choice is snapshotted at admission. Older settings and the
legacy API default to off. The original native UI/thread was not run. Matching is staged before publication:
more than 1,024 registered or 32 active triggers, 10,000 lines, 16 MiB of log
text, a 100 ms regex timeout or a two-second aggregate budget rejects the import
without attaching encounters or emitting staged results. The time budget is
cooperative between regex calls; a call can run up to its 100 ms timeout beyond
the budget, and XML decoding itself has a separate 64 MiB input limit. Accepted
archive work is drained on stop and cannot be canceled after admission. A
rejected matching attempt can be retried after changing the input or settings.
