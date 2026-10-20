# Combat registration and log following

This slice reconstructs registration and raw file delivery. It does not implement
ACT combat aggregation, triggers, historical encounters, or live game capture.
The later [M2 root slice](m2-root-combat.md) adds actual action routing and combat state.

## Supported contracts

| Surface | Behavior |
| --- | --- |
| Combat registries | Store incoming/outgoing categories, swing links, damage/healing swing types, export formatters and column definitions |
| Registered callbacks | Preserve writable delegates, typed arguments, sorting and color callbacks; missing column lookup returns an empty string |
| Headless validation | Reject category links to missing definitions; rebuild category and visible-column views from registered definitions |
| Session lifetime | Replace static registries at host acquisition and shutdown so later sessions do not invoke disposed plugin callbacks |
| Action payload | Explicit-special `MasterSwing` constructor preserves 64-bit `Dnum` identity, time, actors, attack metadata, mutable critical/encounter fields and tag dictionary identity |
| `ReadThreadLock` | Pause file reading and expose the pause state to original plugin workers |
| `OpenLog` | Open the selected file at its end and deliver subsequent complete UTF-8 records |
| Raw hooks | Run synchronous mutable `BeforeLogLineRead` handlers before `OnLogLineRead`; report individual subscriber failures and continue |
| Timestamp state | Use the installed plugin parser; retain the last known timestamp when it returns `DateTime.MinValue` |
| Raw zone snapshot | Search complete UTF-8 records backwards from the reader's starting offset; later appends remain in reader order |
| Log ordering | Increment the writable `GlobalTimeSorter` before raw callbacks |
| Linux `Control.Invoke` | Use a caller-supplied synchronization context, verify owner-thread execution and preserve callback exceptions; a missing or invalid pump fails visibly |
| Rust `Host::pump` | Cooperatively execute queued Linux UI callbacks on the host's creation thread; begin at most 64 callbacks within a 5 ms scheduling budget |
| Reader shutdown | Cancel and join the reader before releasing ACT globals; callback self-disposal avoids joining its own thread |

Registration dictionaries begin empty. Built-in ACT defaults and real combat
objects are not fabricated to make initialization succeed. The legacy-reference
probe uses uninitialized identity objects only to verify callback arguments.
The later [M2 data slice](m2-import-research.md) implements value semantics and
actual encounter relationships. The localized default-special constructor remains absent.

## Input integrity

| Condition | Result |
| --- | --- |
| Existing file content | Excluded from live delivery; opening a file is not an import |
| Partial record or split UTF-8 character | Retained until the newline arrives |
| CRLF and blank lines | Normalize the line ending and preserve complete blank records |
| Malformed UTF-8 record | Report a decoding error, skip that record, continue with later records |
| Pending record exceeds 8 MiB | Report once, discard through its newline, resume subsequent records with bounded buffering |
| File truncated below reader offset | Clear pending bytes and restart from the beginning |
| File replaced or renamed | Continue the original open file until the plugin calls `OpenLog` again |
| File replacement overlaps shutdown | Keep retiring readers tracked and drain their callbacks before releasing host globals; reject further opens if 31 readers are still retiring |
| Reader I/O failure | Report the failure and stop that reader; no success or lossless-recording claim |

The malformed/oversized-record policy concerns raw input integrity. It does not
change the planned bounded Rust subscriber queue policy. Original plugin recording
remains independent; skipped input cannot produce complete downstream analysis.
Truncate-and-regrow past the old offset between polls is not detected.

## Evidence and boundaries

| Evidence | Scope |
| --- | --- |
| `ev_4f0734cd249f7799f13d6796a33cfc17a33b9e7600c7709ec2206032a776cf4c` | Original ACT registration definitions and callback signatures |
| `ev_fe3003dfce77384de05ee30f8d23ed73ae89283e9bd0b175348db68a1c6785fa` | Original worker pause/visibility dependency |
| `ev_162b9a2d7e7c0cb0ef9ee2b7fe53b32a389a0e9729ad6802bc10413b88a5a6fb` | Original FFXIV writer configures the log filter/path and calls `OpenLog(true, false)` |
| `ev_e10bfceb62a78ed060b028138f7963d250457608d5f6551af9718f399d5894c9` | Original explicit-special `MasterSwing` constructor and log sorter property |
| Original-reference probes | Compile signatures against supplied ACT 3.8.5.288 metadata, execute against the replacement |
| Managed reader checks | End-tail framing, split UTF-8, pause/resume, truncation, malformed/oversized recovery, callback isolation and disposal |

Original-artifact runtime checks use a disposable copy of the FFXIV plugin
directory because its startup can modify companion files. A started status alone
does not prove healthy workers, parsing, Linux native capture, or FF Logs file
acceptance. Windows execution and original ACT aggregation comparisons remain
unverified.

| Local runtime observation | Result |
| --- | --- |
| Original plugin | Supplied FFXIV ACT plugin 3.0.3.3, disposable copied directory, Linux/.NET 10.0.13 |
| Initialization | `FFXIV_ACT_Plugin Started.` |
| Worker observation | 1.5 seconds with an explicit owner-thread pump; zero ACT diagnostics; normal version/settings/process startup messages visible |
| Shutdown | Original `DeInitPlugin` and reader shutdown returned successfully |
| Excluded | No game process, imported encounter, real capture, sustained traffic, aggregation comparison or FF Logs validation |

The Linux bridge supplies a synchronization context with a 256-item pending-work
limit. Applications call `Host::pump` regularly; a callback can exceed the
scheduling budget because plugin code cannot be preempted. Queue overload raises
a visible producer error. The dispatcher does not create another UI thread.
Windows retains genuine WinForms and the caller's native UI loop.

Shutdown pumps pending work while joining the reader, before calling plugin
deinitializers. Individual deinitialization temporarily suspends new delivery,
drains the active callback, performs teardown and resumes reading. Lifecycle
operations that would wait for their own dispatched callback fail before changing
host state. A custom managed dispatcher must supply its pump and accurate callback
predicate to `HostEnvironment`; the Rust bridge wires both automatically.
`Host::diagnostics()` returns the atomic total error count and up to 32 bounded
stack traces independently of the last synchronous command. `omitted()` reports
older messages outside the retained history. Repeated queries do not consume
messages. Errors caught privately inside the plugin are not host diagnostics.

## Diagnostic snapshot ABI

This private protocol is separate from the planned live log batches. A fixed
1 MiB payload bounds a cold diagnostic query without borrowing managed memory.

| Offset | Representation | Meaning |
| --- | --- | --- |
| 0 | `u32` | Allocation size, exactly 1,048,608 bytes |
| 4 | `u32` | Version 1 |
| 8 | `u64` | Active session identity |
| 16 | `u64` | Total recorded errors |
| 24 | `u32` | Retained message count, at most 32 |
| 28 | `u32` | Used payload bytes |
| 32 | Repeated `u32 length` + UTF-8 bytes | Messages in recording order |

Each message is capped at 8,192 UTF-16 code units before encoding. Rust validates
counts, lengths, capacity, complete consumption and UTF-8 before producing owned
strings. Empty sessions return zero errors; a restarted host has new history.
Malformed requests cannot consume history or write response fields.
