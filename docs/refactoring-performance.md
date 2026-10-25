# Refactoring measurements

Measured on Linux x64 on 2026-10-25, using .NET SDK 10.0.113 and Rust 1.99.1.
The baseline is commit `d088837`, with the same query benchmark added to its test
runner. No production dependencies or public host interfaces were added.

## Changes

| Module                     | Change                                                                            | Reason                                                                                                   |
| -------------------------- | --------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Combat query encoding      | Write character spans; return the owned stream buffer as an array segment         | Remove per-string UTF-8 arrays and the final payload copy                                                |
| Attack paging              | Count whole categories; enumerate only requested attack indexes                   | Skip work proportional to the number of attacks outside the page                                         |
| ACT event dispatch         | Use typed `Delegate.EnumerateInvocationList`                                      | Remove invocation arrays and casts while retaining order and isolated exceptions                         |
| Encounter aggregates       | Read existing actor collections; filter enemies during ranking                    | Avoid copying lists just to calculate totals or build another list                                       |
| Initial log-zone scan      | Decode the existing byte-list span; reuse the strict encoder                      | Avoid a byte-array copy and encoder allocation per historical record                                     |
| Host audio output          | Share bounded draining and response validation; reuse one packet within each pump | Keep queue/error rules local and avoid up to 31 packet allocations per drain                             |
| Edge text escaping         | Borrow unchanged text; reserve the exact escaped size                             | Avoid five intermediate replacement strings and output-buffer growth                                     |
| Import record reader       | Decode complete records directly from the read buffer                             | Copy bytes only for records spanning reads                                                               |
| Import totals              | Accumulate counters in one checked traversal                                      | Remove an encounter array and three separate aggregation passes                                          |
| Archive operations         | Separate export/restore command handling; attach already parsed records lazily    | Reduce nesting and avoid an intermediate tuple list without weakening atomic restore                     |
| Archive XML                | Compress/write the used stream-buffer segment                                     | Remove a second full-size XML array                                                                      |
| Archive checksums          | Update a table-based IEEE CRC-32 during decompression                             | Remove eight branches per byte and a separate full-output scan                                           |
| Empty archive compression  | Emit a canonical empty gzip member                                                | Fix the existing zero-byte result so empty input round-trips through the public compression methods      |
| Personal duration          | Sum each matched segment directly                                                 | Remove two parallel start/end lists and unreachable imbalance bookkeeping                                |
| Text exports               | Walk StringBuilder chunks to find the 256-line cutoff                             | Remove the split array, per-line strings and concatenation pipeline                                      |
| Plugin loading             | Hash file streams; share dependency lookup order; select contracts directly       | Avoid loading the entire file just to hash it, temporary assembly arrays and duplicate path construction |
| Timer matching             | Normalize each repeated input/category once                                       | Avoid redundant culture-sensitive casing inside loops                                                    |
| Managed audio queues       | Use a named value tuple for effects; share one strict UTF-8 encoder               | Remove queued/empty-result wrapper objects and per-request encoder instances                             |
| Rust result decoding       | Validate borrowed metadata; allocate strings only for public snapshots            | Remove three discarded strings and synthetic revision/status parameters from encounter-detail decoding   |
| Rust original logs         | Share consecutive equal source paths with `Arc<Utf8Path>`                         | Remove per-record path copies and make record clones share paths                                         |
| Rust file playback         | Borrow plain paths and reserve the bounded file size                              | Avoid a PathBuf copy and incremental audio-buffer growth                                                 |
| WinForms trees             | Traverse ownership iteratively and join current path segments once                | Remove recursive traversal and repeated prefix strings                                                   |
| WinForms lookups           | Search control/list/chart collections directly                                    | Remove capturing predicates, LINQ iterators and name-check allocations                                   |
| WinForms disposal/dispatch | Allocate errors/child collections only when needed; invoke Actions directly       | Avoid leaf-disposal bookkeeping objects and reflection for ordinary callbacks                            |

The managed bridge payload embedded in the Rust crate is rebuilt with these
changes. Snapshot ownership, mutable ACT collections, capacity limits, strict
UTF-8 validation and individual subscriber failure handling remain in place.
ACT plugin signatures are unchanged. Rust `LogRecord.source_path` now uses
`Arc<Utf8Path>` instead of `Utf8PathBuf`; see [log ownership](logs.md).
Managed
`HostEnvironment.DrainSound` now returns `(byte[] Bytes, int Volume)` instead of
`Tuple<byte[], int>`; direct managed callers must rebuild for this return type.

## Query workload

The fixture creates one encounter with 512 distinct attacks and two combatants,
ends combat and warms each query 200 times. Each measurement encodes 2,000
queries, checks payload lengths and reports current-thread managed allocations.
Warmup checks byte equality; SHA-256 hashes allow comparison between builds.
The measured operation includes graph queries, DTO creation and binary encoding;
it excludes the native copy into Rust and subsequent Rust decoding.

```sh
DOTNET_TieredCompilation=0 mise run test:query-load
```

Disabling tiered compilation makes allocation totals reproducible. Ordinary
tiered execution produces different totals. Bytes below are decimal MB, summed
over 2,000 queries; they measure allocated traffic rather than retained memory.

| Query                 | Payload bytes | Before allocated MB | After allocated MB | Reduction |
| --------------------- | ------------: | ------------------: | -----------------: | --------: |
| First 256 attack rows |        24,799 |             367.376 |            259.504 |     29.4% |
| Last 16 attack rows   |         1,547 |              25.024 |             17.200 |     31.3% |
| Encounter detail      |           464 |               7.472 |              6.288 |     15.8% |

| Query                 | Baseline SHA-256, identical after refactoring                      |
| --------------------- | ------------------------------------------------------------------ |
| First 256 attack rows | `C53319CCB23C05DD25DB00ACA20AF2FEB7B1E2F73B739697D4DDE95F2397FF74` |
| Last 16 attack rows   | `3D48C87FF7202FD7CC33254D5100BDB74827035722B517C21FBD1E2D5189DF13` |
| Encounter detail      | `16C9D547A32D8CA09D10BEA6769113BAB7C486781D1705C60066EF259DFEE561` |

Two sequential comparisons observed 839–845 ms before versus 804–972 ms after
for the first page, 72–77 ms versus 52–62 ms for the tail, and 26–27 ms versus
24–29 ms for detail. Timing is noisy on this shared machine: only the tail query
consistently improved in these samples. These observations are not a general
throughput guarantee. No timing or allocation threshold makes the tests flaky.

## Archive workload

```sh
DOTNET_TieredCompilation=0 mise run test:archive-load
```

The fixture repeats all 256 byte values across a 2 MiB payload, compresses it
once, warms decompression eight times, then decompresses and compares every byte
32 times. The baseline uses the same benchmark on `d088837`; these archive paths
were unchanged by the initial query refactor.

| Metric                     |      Before |       After |
| -------------------------- | ----------: | ----------: |
| Total elapsed milliseconds |     1,532.5 |       210.1 |
| Managed allocated bytes    | 201,347,072 | 201,347,072 |
| Successfully decoded bytes |  67,108,864 |  67,108,864 |

This synthetic sample improved elapsed time by about 7.3 times; it does not
measure filesystem I/O or XML graph reconstruction. Allocation traffic is
unchanged for this workload. Both outputs have SHA-256
`91D3BEB88A9B2F778A6C44A1C53B63D3C79931845A9AEF84B3FB414610BD1938`.

## Headless WinForms workload

```sh
DOTNET_TieredCompilation=0 mise run test:winforms-load
```

The independent Forms contract runner warms each operation 200 times and checks
its result while measuring current-thread managed allocations. The baseline is
the same runner added to `d088837`; its WinForms implementation was unchanged by
the earlier passes. Setup is excluded. Disposal includes constructing the leaf
control itself. No ACT, GUI, native rendering or timing threshold is involved.

| Operation                    | Iterations | Before allocated bytes | After allocated bytes | Before ms | After ms |
| ---------------------------- | ---------: | ---------------------: | --------------------: | --------: | -------: |
| Control name lookup          |    100,000 |              8,800,000 |                     0 |      6.75 |     0.99 |
| Chart rename and name lookup |    100,000 |             26,400,000 |                     0 |     13.76 |     1.43 |
| Full path at depth 128       |      1,000 |             85,472,000 |             3,496,000 |     38.45 |     3.16 |
| Leaf construction/disposal   |    100,000 |             33,600,000 |            14,400,000 |     18.56 |     8.79 |

Tree-path allocation traffic decreased 95.9%; leaf construction/disposal traffic
decreased 57.1%. These are synthetic observations on this machine. Control
disposal still snapshots existing children before invoking arbitrary callbacks;
removing that copy would change sibling cleanup during reparenting callbacks.

## Verification and limits

| Check                              | Scope                                                                                                                                                                                                               |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mise run check`                   | Managed/WinForms contracts, real CLR lifecycle and fixture plugins, Rust format, strict workspace Clippy, workspace tests and doctests                                                                              |
| Attack paging regression           | Compare all small-fixture offsets at limits 1, 2, 3 and 256, including category transitions and empty pages                                                                                                         |
| Query encoding regression          | Independently read binary fields, Unicode/NUL text, long text across encoder buffers, invalid surrogate rejection, exact segment length and ownership after a later encoding                                        |
| Audio response regression          | Every truncated prefix, invalid UTF-8, invalid volume and empty queue; existing CLR fixtures cover multiple speech/effect requests and failure order                                                                |
| Edge escaping regression           | Every XML escape, existing entity text, Unicode and borrowed ordinary text                                                                                                                                          |
| Archive regressions                | Independent `gzip -n` known-answer/empty members, public empty compression/decompression round trip, all byte values across read chunks, corrupt CRC/size, truncated footers and identical XML/gzip export contents |
| Import/text regressions            | Direct versus fragmented Unicode/empty records; LF/CRLF, long text and the 255/256/257-line cutoff                                                                                                                  |
| `mise run test:original`           | Replacement and genuine ACT reference contracts, including segmented/negative durations, malformed segments, caches, action aggregation and ally queries                                                            |
| `mise run test:encounter-archives` | ACT XML export/import and history-record probes against both implementations                                                                                                                                        |
| Rust ownership regressions         | Metadata pointers borrow validated packet bytes; combatants outlive the packet; equal adjacent paths share an Arc and outlive the batch                                                                             |
| WinForms regressions               | Depth-2048 ownership/path reads, renamed ancestors/separators, detached subtrees, disposal-time sibling reparenting, exact callback exception identity and existing chart/list contracts                            |

The Linux image had runtime audio libraries but lacked development headers.
For full workspace verification, `libasound2-dev` and `libspeechd-dev` were
downloaded and extracted under `/tmp/aftercast-native-dev`, without installing
system packages. `PKG_CONFIG_PATH`, `PKG_CONFIG_SYSROOT_DIR`,
`BINDGEN_EXTRA_CLANG_ARGS` and `LIBRARY_PATH` pointed the build there.

These passes do not establish Windows behavior, original-plugin/live-game
performance, actual speaker output or Edge network availability. Native audio
and network tests remain opt-in. The independent headless WinForms implementation
is verified through its contract suite; native Windows WinForms remains outside
these Linux runs.
