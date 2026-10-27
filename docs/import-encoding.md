# Source import text encodings

## ACT evidence and compatibility boundary

The pinned ACT 3.8.5.288 binary exposes `FormActMain.LogEncoding`, initialized to
`Encoding.UTF8`. Its main import path and companion reader construct
`StreamReader(stream, LogEncoding)`. The original live reader uses the same
constructor, but the aftercast live follower is outside this import change.

`LogEncodingProbe` compiles against the genuine ACT property and executes against
both assemblies. It verifies selection, Unicode BOM override, CR/LF and default
UTF-8 replacement using the inspected StreamReader constructor contract. It also
checks two non-obvious boundaries: the selected encoding's own preamble takes
precedence over BOM detection, and is consumed even when detection is disabled.
Thus UTF-16LE selection consumes the first two bytes of a UTF-32LE BOM and keeps
UTF-16LE; it does not always switch to UTF-32. This is a constructor/data contract
probe on Linux, not execution of ACT's native import dialog or Windows Framework.

## Implemented managed import contract

`HostEnvironment.ImportLog` and `BeginImport` accept optional `ImportTextOptions`.
Omitting options snapshots the plugin-selected `FormActMain.LogEncoding`; the
replacement root defaults to strict UTF-8. The existing Bridge/Rust full/range
routes already inherit a plugin's selection, but explicit request options and GUI
selection remain a subsequent connection unit.

| Code page     | Explicit encoding          |
| ------------- | -------------------------- |
| 65001         | UTF-8                      |
| 1200 / 1201   | UTF-16 little / big endian |
| 12000 / 12001 | UTF-32 little / big endian |
| 932           | Windows Shift-JIS          |
| 949           | Windows Korean             |
| 1252          | Windows Western European   |
| 20127         | ASCII                      |

These encodings have no escape-mode state across records. The existing UTF-8/DBCS
byte framing uses CR/LF bytes; UTF-16/32 framing requires complete aligned code
units. Split code units, DBCS sequences and surrogate pairs stay in the pending
record across short reads. Decode only a complete record, so a malformed suffix
does not discard earlier complete records in the same read buffer. Other code
pages, including locale alias 0 and stateful ISO-2022, are rejected before import
admission; no locale/byte-content guess or external converter is used. Code pages
come from the installed .NET framework provider, registered once for plugin use.
Custom Encoding implementations are not a tested compatibility guarantee.

| Policy                   | Behavior                                                                                                                                                                                                                                                                                       |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CodePage`               | Must be one of the explicit values above. The admitted reader owns a clone, so later plugin changes cannot alter its interpretation.                                                                                                                                                           |
| `ReplaceInvalid = false` | Strict decoding; invalid bytes, truncated units and invalid Unicode scalars fail visibly. This preserves aftercast's strict UTF-8 default.                                                                                                                                                     |
| `ReplaceInvalid = true`  | Use the selected framework encoding's standard decoder fallback, matching ACT's normal replacement behavior; the replacement is encoding-dependent.                                                                                                                                            |
| `DetectBom = true`       | Consume the selected preamble first. Otherwise recognize byte-zero UTF-8/16/32 BOMs, checking UTF-32 before UTF-16. Interior U+FEFF is ordinary record data.                                                                                                                                   |
| `DetectBom = false`      | Disable detection of other Unicode encodings. The selected encoding's own preamble is still consumed, as with StreamReader.                                                                                                                                                                    |
| Strictness after BOM     | Preserve an explicitly strict fallback after switching encoding. ACT's StreamReader may reset fallback to the detected encoding default when it switches. This difference is intentional; BOM detection cannot silently weaken strict mode.                                                    |
| Custom fallback          | Reject unknown fallback implementations, more than two replacement characters, or replacement CR/LF. Standard code-page fallbacks remain supported. This bounds expansion and prevents replacement-generated line delimiters from bypassing byte framing.                                      |
| Bounds                   | Retain the 8 MiB source-byte record limit, excluding BOM and delimiters; at most one partial UTF code unit is buffered while locating a delimiter. A strict decode failure makes that reader non-resumable.                                                                                    |
| Lifecycle                | Full and inclusive-range imports share selection/framing. Byte progress remains source bytes. Cancellation, captured file length, valid prefix/action draining and live-context restoration reuse the existing pipeline. Each import gets a fresh reader; no decoder state crosses operations. |

New synthetic tests cover Korean/Japanese and supplementary Unicode, nine code
pages, mixed CR/LF and one-byte/7-byte/4096-byte reads, all Unicode BOMs and their
selected-encoding/detection cross-product, BOM-only input, interior BOMs, delimiter
look-alike code units, strict/replacement failures, dangling code units, cancellation
before reading, maximum records and refusal to resume after strict failure.
Pipeline tests verify full/range action totals, equal-time bounds, captured plugin
selection, progress bounds, cancellation cleanup, and damage retained before a
malformed suffix. They use only temporary synthetic files.

## Remaining connection and validation

Bridge/Rust explicit options, import/reparse GUI selection and request ownership
regressions remain pending. Existing requests keep their previous default rather
than silently enabling replacement. Companion registration/dispatch and source
matching remain separate finite tasks. Windows native import, legacy Framework
code-page equivalence and actual plugin/UI behavior remain unexecuted. This does
not claim arbitrary .NET encodings or live-follower encoding parity.
