# Explicit clipboard text import

## ACT evidence

The pinned ACT 3.8.5.288 artifact has SHA-256
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
The existing `OriginalReference import-policy` probe now checks ImportClip IL,
without invoking its clipboard APIs or native import thread.

| Original ImportClip location | Observed contract                                                                                        |
| ---------------------------- | -------------------------------------------------------------------------------------------------------- |
| IL_0192 / IL_0199            | ContainsText followed by GetText, only in the explicit import operation; absence shows an error          |
| IL_0231 / IL_027a            | StringReader over Unicode text, TextReader.ReadLine; no LogEncoding conversion or BOM stripping          |
| IL_0283 / IL_0348–IL_034d    | Empty physical rows are not dispatched; more than 1000 consecutive empty reads ends the operation        |
| IL_02b6–IL_02d6 / IL_032d    | A dated row updates LastKnownTime; undated rows inherit the existing time, including a first undated row |
| IL_032a–IL_0339              | ParseRawLogLine(import=true, LastKnownTime, line, empty companion name), with a distinct global sorter   |
| IL_0405                      | EndCombat and ordinary import completion                                                                 |

## Managed input checkpoint

`HostEnvironment.BeginClipboardImport(string)` accepts an immutable Unicode
snapshot. It never reads/writes an OS clipboard, watches clipboard changes,
creates a source file or searches for companions. Bridge/Rust admission is connected as described below. GUI remains a separate
next unit; the application will read only after an explicit user
paste/import action through an injectable adapter. No native clipboard was used
by this checkpoint's tests.

The existing import lifecycle owns reader suspension, bounded producer channel,
owner-thread parsing, action drain, partial history, cancellation, progress and
live-context restoration. Admission requires ending live combat, as with existing
aftercast imports; this unit does not add ACT's active-live clipboard admission.
A small input-reader interface shares scheduling with file/captured imports while
keeping text separate from byte decoding.

| Boundary       | Contract                                                                                                                                                                                                                                                                                                                                                         |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Text budget    | Nonempty well-formed Unicode, at most 8 MiB when encoded as strict UTF-8. This is an aftercast fixed resource limit, not a discovered ACT limit. Invalid UTF-16 and over-limit input fail before import admission; no replacement is guessed.                                                                                                                    |
| Encoding       | Clipboard is already decoded text. Plugin LogEncoding, raw-file encoding/BOM policies, time bounds, combat extension and companion selections do not apply. U+FEFF remains a text character; parsers receive it.                                                                                                                                                 |
| Framing        | StringReader handles CR, LF, CRLF and final unterminated records. Empty rows are skipped by dispatch. The ACT >1000 consecutive-empty threshold is preserved, so a later suffix can remain unconsumed.                                                                                                                                                           |
| Clock          | Clipboard preserves the pre-import LastKnownTime for a leading undated row. Later dated rows update it; later undated rows inherit it. Existing file/capture clock initialization remains unchanged.                                                                                                                                                             |
| Progress       | Existing BytesRead/TotalBytes fields describe exact UTF-8 equivalent bytes of the Unicode snapshot, including its actual terminators; no disk read is implied. LinesRead counts consumed physical rows, including empty rows. This differs from ACT's nonempty-line count and assumed two-character terminator estimate. Progress can precede callback dispatch. |
| Empty content  | Missing/empty text is a visible input error before mutation. Nonempty all-blank text completes with no encounters.                                                                                                                                                                                                                                               |
| Failure/cancel | Accepted actions drain and remain as partial imported history. The original error and cleanup errors retain the existing failure contract. Live path/zone/time are restored and a fresh import is allowed afterward.                                                                                                                                             |
| Provenance     | During callbacks the source path and companion name are empty. The old live filename is never reported as this text's source. Encounter FolderHint stays empty, and private history records an AftercastClipboardImport marker. The original ACT reuses its existing LogFilePath in ImportClip; aftercast deliberately avoids that ambiguous path attribution.   |
| Persistence    | Private history preserves the clipboard marker, retained log/action sorter links and empty folder hint after restart. It retains the ordinary bounded encounter logs, not the entire clipboard or a fictitious source file. The marker is omitted from public ACT XML interchange, whose existing identity limits still apply.                                   |

Restoring a checkpoint reconstructs its saved graph/logs. It does not read the
clipboard again or offer a fictitious source-file replay. An explicit new file
candidate remains a metadata proposal rather than proof of historical identity.
Existing retained-log limits and omission counters still apply. Invalid XML
characters or ordinary archive I/O errors are reported by existing save handling;
clipboard import does not silently sanitize source text or promise every accepted
text can be represented in ACT XML.

`ClipboardImportChecks` uses only synthetic strings and temporary archives. It
covers Korean/Japanese/emoji, BOM preservation, CR/LF/CRLF, final records,
standalone/leading/following undated rows, ASCII-plugin independence, empty and
malformed Unicode, exact/over-limit ASCII and multibyte budgets, duplicate
admission, both sides of the empty-row threshold, progress, partial parser
failure, explicit cancellation/reuse, pathless callbacks, private save/restart,
provenance and action/log identity. Managed import regressions include it.

## Bridge/Rust checkpoint

`HostHandle::start_clipboard_import(content)` copies a validated Unicode string
before enqueueing; `import_clipboard_text(content)` awaits the same ImportJob.
`MAX_CLIPBOARD_IMPORT_BYTES` exposes the fixed 8 MiB UTF-8 byte budget. Rust strings
are already well-formed Unicode, so no encoding guess or invalid-byte policy is
applied. File, Clipboard and Captured inputs have distinct internal enum variants;
clipboard never passes through path canonicalization or receives raw-file options.

| Wire boundary           | Contract                                                                                                                                                                                                                                              |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ImportOperation Begin 5 | A 64-byte header followed by 1..8 MiB strict UTF-8 text. Offset 16 is exactly 64; length at 20 equals packet length minus 64. Timestamp bounds, encoding/flags, owner-output and reserved fields remain zero.                                         |
| Admission               | Validate capacity, ABI/session, exact ranges, reserved fields and UTF-8 before owner-thread Begin. Requires an initialized plugin. Copy text before returning; the native pointer is never retained. Embedded NUL is text, not a filename terminator. |
| Advance/Cancel 2/3      | Reuse existing 64-byte option-free import continuations, progress, cancellation and action drain. Begin cannot replace an active operation.                                                                                                           |
| Existing file/capture   | File packet capacity stays 64 KiB, and existing file options/ranges are unchanged. Captured source owners still use the History command and owned source continuation.                                                                                |
| Lifetime                | Observer drop and dropped queued admission responses do not cancel accepted work. Explicit cancel and Host shutdown/drop drain through the existing import operation; stale closed handles cannot submit to a restarted session.                      |

The actual isolated-worker `clipboard_import` regressions use a synthetic parser
and synchronization markers under a temporary Host directory. They cover copied
Unicode/BOM/CR/LF text, both exact 8 MiB ASCII/multibyte requests across the binary
transport, over-budget/empty rejection, consecutive-empty cutoffs, leading
undated time, UTF-8-equivalent progress, original records with an empty source
path, partial parse errors, cancel, observer/admission drop, shutdown/drop and
restart. They also export ACT XML without the private origin marker and restore
private history with the same log/sorter links and empty FolderHint.

Managed tests separately verify that the private clipboard marker survives
restore. The worker test verifies existing immutable archive payloads remain
unchanged and are not deleted when resaving. Whole-archive byte identity after
resave is not a contract: existing import-zone serialization can normalize
`ZoneName="Import"` to `ZoneName="Import Zone"` after restore. This baseline
representation difference does not change the tested clipboard marker or
retained actions/logs and was not changed as part of this unit.

## Supplied-provider GUI checkpoint (OS wiring incomplete)

The Import / Export pane registers the existing Rust/UI Accordion, Button and
Progress components for an explicit read → preview → import flow. The adapter is
injected through Dioxus context. The default provider is deliberately unavailable,
so the normal application's Paste button is disabled and explains that no provider
is connected. **This is not completion of the end-user OS clipboard feature.**

A live `arboard` reader addition was rejected by automatic approval review before
execution. No such reader was added, invoked or substituted through another OS
path. Only the unaffected supplied-provider UI/controller and in-memory tests
were completed. OS wiring remains an approval blocker, rather than a claim that
Linux clipboard access is technically impossible.

| GUI contract    | Implemented supplied-provider behavior                                                                                                                                                                                                                                                                                                                                                                  |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Explicit action | Construction, render, tab changes and import never read again. Only Paste requests a provider snapshot; Import uses that immutable preview. No watcher or persistent OS handle is implemented.                                                                                                                                                                                                          |
| Limits/preview  | Validate nonempty UTF-8 equivalent size up to 8 MiB before staging. Show byte/character counts and at most 512 characters, keeping complete snapshot bytes for import. Unsupported formats and unavailable/busy providers are explicit local failures; previous draft is retained.                                                                                                                      |
| Ownership       | One provider read at a time, including across Host session changes. Late results cannot replace a newer draft/session. Duplicate read/import/clear/cancel events are rejected.                                                                                                                                                                                                                          |
| Quit            | Abandon a pending read token without waiting for its result, then use the existing Ready→Host Stop or Idle→window-close path. A late supplied result is ignored; no unsafe thread termination is attempted.                                                                                                                                                                                             |
| Import          | Reuse typed clipboard API and tab-independent wait/cancel. Raw path/encoding/range/companion drafts are separate. Failed/cancelled snapshots remain for inspection/retry; Host stop retains the draft and a fresh application starts empty.                                                                                                                                                             |
| Privacy         | No clipboard text/provider diagnostic is formatted into shared desktop errors, application logs or settings. Adapter errors carry no strings. Import failures show a fixed local message. Preview is intentionally visible and imported text intentionally reaches initialized parsers and bounded retained encounter logs; third-party plugin diagnostics remain governed by existing plugin behavior. |

Four tests cover in-memory Unicode/size/format failures, a single-flight blocking
provider with stale-session replies, pending-read Quit in Idle/Ready, and an actual
isolated Host controller flow. The Host scenario checks immutable preview use,
raw option independence, tab navigation, partial parsing failure, draft retention,
general desktop log redaction with an active captured tracing sink, cancel and
late session results. No test calls a production/OS reader; all read operations
use injected memory data.

## OS support evidence and remaining wiring

The project already depends on registry `arboard` 3.7.0 with default features off.
Its installed official crate source exposes `Clipboard::get_text`, a Windows
backend and an unconditional Linux X11 backend. Native Wayland data-control is
behind an optional feature that is not enabled here; an X11/XWayland connection
would use the existing path. No new installation, dependency, persistent grant or
OS setting change was required by the supplied-provider work. The proposed live
read shim was not added or compiled because of the approval rejection; this
checkpoint makes no live-provider build or execution claim.

1. Obtain approval for adding/connecting a live provider without executing OS
   reads in tests, then replace the default unavailable provider. Keep tests
   explicitly in-memory; do not invoke a default provider read as a test.
2. Actual OS clipboard access and native UI input/Windows execution remain
   unverified. Neither IL policy probes nor synthetic Host tests run native ACT
   clipboard code. Clipboard monitoring remains outside this feature.
