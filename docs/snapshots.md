# Combat snapshots

The Rust host can query the latest ended encounter without exposing managed
objects or duplicating ACT aggregation. Original-log subscriptions remain
independent of this result path; subscriber overflow does not discard combat
actions.

## Consumer contract

| Operation or value                            | Meaning                                                                                                                          |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `host.combat_snapshot(None)`                  | Latest ended encounter, or `None` before any completion                                                                          |
| `host.combat_snapshot(Some(result.revision))` | Latest newer result, or `None` when unchanged                                                                                    |
| `host.end_combat()`                           | Explicitly ends live combat after quiescing file callbacks and draining admitted actions; idle calls preserve the current result |
| `CombatRevision`                              | Opaque, session-scoped monotonic state version; rejected after host restart                                                      |
| `superseded_versions`                         | Intermediate result/status versions since the supplied revision; not encounters or lost input                                    |
| `Ended`                                       | Core ended combat and any enclosing import completed without a known host failure                                                |
| `Importing`                                   | Core ended this encounter while its enclosing import is still running                                                            |
| `PartialImport`                               | Partial history retained after a known input, hook or aggregation failure                                                        |
| Query failure                                 | Malformed ACT state or a result exceeding bounds fails visibly; core parsing continues                                           |

```rust,no_run
fn read(host: &mut aftercast::Host) -> Result<(), aftercast::Error> {
    if let Some(result) = host.combat_snapshot(None)? {
        println!("{}: {} damage", result.title, result.damage);
        let revision = result.revision;
        // Pump owner callbacks before checking whether a newer result has been published.
        host.pump()?;
        if let Some(next) = host.combat_snapshot(Some(revision))? {
            println!("{}: {} damage", next.title, next.damage);
        }
    }
    Ok(())
}
```

Only the latest ended encounter is retained for `combat_snapshot`. For active
combat and retained history, use the GUI queries below. Callbacks and durable
storage are not implemented. A later empty import
does not replace an earlier result. A failed import can replace it with partial
history. Completion status does not establish that the plugin accepted every raw
record or flushed its private deferred effects.
Explicit completion also does not flush the original writer or plugin-private
deferred effects. File delivery resumes after the boundary, including after
failure; use this command before importing while live combat is active.

## Values and ownership

| Scope            | Data                                                                                                                                                 |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Encounter        | Zone, title, character, original start/end ticks and kind, signed duration ticks, allied damage/healing totals                                       |
| Every combatant  | Name, ally membership, damage/healing given/taken, duration, DPS/encounter DPS/HPS, hit percentage, hit/critical/heal/miss/block/kill/death counters |
| Ownership        | Copied managed values at completion; Rust strings and vectors survive imports and host stop                                                          |
| Numeric fidelity | Signed ACT counters, NaN, infinity and negative zero are preserved; no Rust recalculation or clamping                                                |
| Time fidelity    | 100 ns ticks since year 1 and original `DateTimeKind`; graph-local timestamps have no inferred UTC offset                                            |

Publication follows the combat-end callbacks. The result is frozen so later
plugin edits to the mutable ACT object graph cannot silently change the same
revision. Import completion changes only the frozen result's status and version.
A copy failure is retained for query instead of interrupting parsing. Managed
query callers receive a separate actor array as well.
Strings preserve plugin values. The pinned plugin receives the Linux
[CRLF resource correction](compatibility.md#linux-corrections); snapshot encoding itself
does not trim or rewrite territory names.

## GUI combat queries

| API | Returned data | Idle / removed behavior |
| --- | --- | --- |
| `live_combat()` | Active live encounter and participant counters, including DPS/HPS | `None` while idle or importing |
| `encounters(offset, limit)` | Total count and a page of encounter summaries | Empty page when offset is past the end |
| `encounter_detail(id)` | Selected encounter summary and participant counters | `None` if a plugin removed it |
| `encounter_attacks(id, offset, limit)` | Category/attack totals, direction, damage, duration, DPS, hits/criticals/misses/blocks | `None` if removed; otherwise possibly an empty page |

```rust,no_run
fn refresh(host: &mut aftercast::Host) -> Result<(), aftercast::Error> {
    // Service plugin callbacks before refreshing UI data, e.g. on an application timer.
    host.pump()?;
    if let Some(live) = host.live_combat()? {
        println!("{}: {} damage", live.summary.title, live.summary.damage);
        for actor in live.combatants {
            println!("{}: {} DPS", actor.name, actor.enc_dps);
        }
    }
    let page = host.encounters(0, 50)?;
    if let Some(row) = page.items.first() {
        let detail = host.encounter_detail(row.id)?;
        let attacks = host.encounter_attacks(row.id, 0, 100)?;
        println!("{detail:?} {attacks:?}");
    }
    Ok(())
}
```

| Contract | Meaning |
| --- | --- |
| Consistency | Each call copies graph values under the ACT combat lock on the host owner thread; separate calls/pages can observe later state |
| Freshness | Includes already-applied actions; does not drain producers or end combat. Pump regularly and poll at a modest rate, e.g. 5 Hz |
| Duration | Based on ACT combat records and segments, not elapsed UI wall time; rates retain ACT NaN/infinity behavior |
| Identity | Opaque `EncounterId`, stable through combat completion and later imports; another host session rejects it |
| State | `EncounterStatus`: `Active`, `Ended`, `Importing` or `PartialImport`; active live combat is distinct from completed combat |
| History | Current session's retained ACT graph, including repeated imports and partial results; excludes zone `All` aggregates and duplicate object references |
| Order | ACT zone/list order, with imported encounters first; not a global chronological sort. Attack rows follow combatant/category/attack key order |
| Pagination | Offset and limit, 1..256 rows per page; offset must fit signed 32 bits. Refresh the list when graph changes move offsets |
| Import status | New imports are `Importing` until restoration completes. Failure marks every encounter added by that import `PartialImport`; earlier imports keep their status |
| Ownership | Strings, participant arrays and attack rows remain valid after later input and host shutdown; subsequent queries read current plugin edits |
| Aggregates | `All` attack rows and overlapping ACT categories are returned verbatim. Do not sum every row to compute encounter damage |
| Scope | Participant and attack aggregates; individual swing/log inspection and persistence across process restart are outside these queries |
| Bounds | 65,536 participants in a detail response and 64 MiB per encoded query. Invalid state or oversize results report an error |

Unlike `combat_snapshot`, these queries read the mutable graph each time and
provide no frozen revision or change notification. They do not initialize an ACT
environment when none exists. Removed graphs are not kept alive just for query IDs.

### GUI query ABI

`QueryCombat` uses a separate 48-byte, version-1 little-endian header. Status 0
means success, 1 requests a larger capacity, and -1 reports a caught error. A
retry reads the graph again; no records are consumed.

| Offset | Field |
| --- | --- |
| 0, 4 | u32 capacity, version 1 |
| 8 | u64 session |
| 16, 20 | u32 mode (1 live, 2 list, 3 detail, 4 attacks), offset |
| 24 | u64 encounter ID, zero for live/list |
| 32 | u32 page limit |
| 36, 40, 44 | u32 payload used, required total capacity, reserved zero; initially zero |

Text is u32 UTF-8 byte length followed by bytes. Times are i64 ticks plus u32
kind. List payloads start with u32 total/count; optional detail/attack payloads
start with u32 presence (0 or 1). Summaries contain ID, flags (import bit 0,
`EncounterStatus` in remaining bits),
zone/title/character, start/end, signed duration/damage/healed and participant
count. Detail then includes a u32 byte length and the existing snapshot payload.
Attack pages contain total/count followed by combatant/category/attack strings,
u32 outgoing, i64 damage/duration, f64 DPS/EncDPS and five i32 counters.

## Private snapshot ABI

`ReadCombatSnapshot` uses the standard native delegate and a 64-byte header.
Requests and results are little endian. Output fields must initially be zero.
All capacities are validated before querying; insufficient capacity returns 1
without consuming a result. Success returns 0; caught errors return -1 and the
ordinary host diagnostic. This is private implementation detail.

| Offset | Field                                                                         |
| ------ | ----------------------------------------------------------------------------- |
| 0, 4   | u32 capacity, version 1                                                       |
| 8, 16  | u64 session, requested previous revision (0 for latest)                       |
| 24     | u64 returned revision, 0 for no result                                        |
| 32, 36 | u32 actor count, flags: present bit 0, import bit 1, status in remaining bits |
| 40, 44 | u32 payload used, required total capacity (only for retry)                    |
| 48, 56 | u64 superseded versions, reserved zero                                        |

The payload contains an 80-byte encounter header, contiguous UTF-8 zone/title/
character strings and actor records with 120-byte headers plus UTF-8 names.
Embedded NULs in ACT strings are preserved. Each actor has checked length/offset
fields and reserved zeros. The bridge validates request versions. Rust rejects
malformed UTF-8, timestamps, flags, offsets, counts and unexpected trailing payload bytes before returning
owned values. Limits are 65,536 actors and 64 MiB of encoded payload.

## Verification boundary

Checks cover completed and partial imports, polling/coalescing, ownership,
malformed ABI data, capacity retries and snapshot failure recovery. The original
FFXIV synthetic fixture checks a 400-damage, two-second, two-combatant result with
the actual plugin. It is a controlled parser/transport fixture, not representative
gameplay, native capture, throughput evidence or FF Logs acceptance.

GUI query checks run in the managed `bridge`/`contracts` modes and the existing
Rust `snapshots` integration scenario. Windows .NET 10 execution covers live
updates before combat end, stable IDs, retained multi-import history, importing
and partial-import states, removed encounters, paging, strict framing, signed and
nonfinite values, ownership and stale-session rejection. Linux execution and
large-session polling performance have not been measured for these new queries.
