# Bounded original-log batches

The Rust host supports one opt-in original-log subscription. It observes the
original text of nonempty records selected for live/import hook dispatch, before
mutable plugin hooks run. It does not replace the original writer, acknowledge
parser acceptance or provide a durable recording.

## Consumer contract

| API / value | Behavior |
| --- | --- |
| `subscribe_logs(LogSubscriptionConfig)` | Starts one subscription; default limits are 4,096 records and 8 MiB of encoded record bytes |
| `drain_logs(subscription)` | Returns an owned batch immediately; capacity retries do not consume records or loss state |
| `unsubscribe_logs(subscription)` | Explicitly discards unread data and retires the identity |
| `LogRecord.original_line` | Valid UTF-8 `Bytes`, including embedded NUL; clones share the Rust allocation, never CLR memory |
| Record metadata | Import flag, source path, original CLR timestamp/kind, captured UTC ticks when known, ACT time sorter and a separate subscription sequence |
| `LogBatch.dropped` | Exact count and contiguous sequence range omitted since the previous successful drain; inspect it even when no records remain |
| Host stop / restart | Discards unread data; old host/subscription identities are rejected, and a new subscription starts its sequence at one |

Drain regularly, for example after pumping owner callbacks, until a batch has no
records. Only the host's creation thread calls the native bridge; owned results
can be retained or moved to consumer threads. There is no callback into Rust from
the parser and no mandatory async executor.
Retaining one line slice can keep its entire Rust batch allocation alive; the
managed queue limits do not bound data deliberately retained by the application.

## Overflow and ownership

| Condition | Result |
| --- | --- |
| Record or byte limit reached | Drop oldest pending observations until the new record fits; continue core parsing and aggregation |
| A single record exceeds the configured byte limit | Drop the pending prefix and the oversized record itself, then accept later records; this preserves an exact missing interval |
| Queue empty after overflow | Retain loss state separately until a successful drain reports it |
| Insufficient native output capacity | Report required capacity, preserve queue/loss, retry with a bounded larger Rust allocation |
| Encoding/allocation failure | Preserve pending records and loss acknowledgement; report the command failure |
| Optional capture observer throws | Retain a host diagnostic and continue mutable hooks/import processing |
| Parser mutates displayed text | The subscription retains the original immutable input text |
| Local timezone changes while buffered | Preserve the UTC instant calculated at capture, alongside the original local ticks/kind |

Allowed limits are 1–65,536 records and 56 bytes–64 MiB of encoded record storage.
The byte bound includes each record header and UTF-8 path/text; managed strings
and queue objects add bounded overhead. Retained ACT history is a separate memory
consumer. Capture/drain use a monitor, and encoding a drain batch briefly holds
that monitor; this is not a lock-free or measured-throughput claim. A slow or
absent consumer never causes a wait for consumer progress.

Source sequences are independent of the public, plugin-writable ACT sorter.
Successful drains acknowledge loss atomically with consuming their record prefix.
Unsubscribe/stop deliberately discard unread data. Observations cannot restore a
recording after subscriber overflow; FF Logs integration must use the original
writer's file, as specified in the [blueprint](blueprint.md).

## Private batch ABI

The existing ComponentEntryPoint convention and borrowed native allocation remain
unchanged. This is an internal Rust/C# protocol, not a stable public C API.

| Header | Bytes / fields |
| --- | --- |
| Command header | 64 bytes: capacity/version, owning session, subscription identity, command, count, first/last dropped sequence, dropped count, used bytes, required capacity |
| Record header | 56 bytes: record size, import/kind flags, sequence, CLR/UTC ticks, signed ACT sorter, path/text offset-length pairs, reserved zero |
| Variable payload | Canonical UTF-8 path then original text; native pointers are retained only for the call |

Rust checks capacities, ranges, UTF-8, reserved flags, timestamp bounds, contiguous
sequences, loss arithmetic and trailing bytes before exposing a record. CLR ticks
are 100 ns intervals since year 1, not Unix timestamps. Unspecified timestamps
have no invented UTC offset. The bridge encodes managed strings, copies the batch
into Rust-owned storage, and shares line slices from that allocation; `bytes`
does not make the CLR transfer zero-copy.

## Verification

| Check | Scope |
| --- | --- |
| Capture seam | Live/import metadata, Unicode/NUL, before-mutation ordering, reentrant source/sorter changes and observer isolation |
| Managed buffer | Count/byte overflow, oversized records, exact intervals, partial drains, no consumption on capacity failure, malformed text rollback, concurrent capture/slow drain, queue bounds and subscription reuse |
| Timezone regression | Linux capture under Asia/Seoul followed by UTC drain retains the original UTC instant |
| Rust decoder | Truncated/corrupt batches, UTF-8, offsets/counts/timestamps, empty loss reports, capacity bounds and shared owned line slices |
| Real CLR integration | Core totals remain 400/450 despite subscriber loss; large-record retry consumes once; stale identities fail and owned bytes survive shutdown/restart |
| Original FFXIV fixture | Repeated seven-record imports with a two-record queue preserve 400 damage and the original `IncToHit` callback while reporting the exact five-record omission |

Sustained gameplay throughput, native capture, disk recording durability, combat
snapshot subscriptions and multiple independent subscribers remain outside this
first log-batch slice.
