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
creates a source file or searches for companions. Bridge/Rust admission and GUI
are separate next units; the application will read only after an explicit user
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

## Remaining L6b wiring

1. Bounded Bridge/Rust Unicode admission with the existing import continuation,
   cancellation and shutdown lifecycle; actual isolated worker regressions.
2. Rust/UI explicit paste/import controls using an injectable clipboard adapter,
   with a snapshot independent of raw-file options and session-owned async state.
3. OS clipboard/native UI interaction requires later Windows/native QA. No
   clipboard access or native ACT execution is claimed by the IL probe.
