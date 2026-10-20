# M3: owned combat snapshots

The Rust host can query the latest ended encounter without exposing managed
objects or duplicating ACT aggregation. Original-log subscriptions remain
independent of this result path; subscriber overflow does not discard combat
actions.

## Consumer contract

| Operation or value | Meaning |
| --- | --- |
| `host.combat_snapshot(None)` | Latest ended encounter, or `None` before any completion |
| `host.combat_snapshot(Some(result.revision))` | Latest newer result, or `None` when unchanged |
| `host.end_combat()` | Explicitly ends live combat after quiescing file callbacks and draining admitted actions; idle calls preserve the current result |
| `CombatRevision` | Opaque, session-scoped monotonic state version; rejected after host restart |
| `superseded_versions` | Intermediate result/status versions since the supplied revision; not encounters or lost input |
| `Ended` | Core ended combat and any enclosing import completed without a known host failure |
| `Importing` | Core ended this encounter while its enclosing import is still running |
| `PartialImport` | Partial history retained after a known input, hook or aggregation failure |
| Query failure | Malformed ACT state or a result exceeding bounds fails visibly; core parsing continues |

```rust,no_run
# fn read(host: &mut bxivact::Host) -> Result<(), bxivact::Error> {
if let Some(result) = host.combat_snapshot(None)? {
    println!("{}: {} damage", result.title, result.damage);
    let revision = result.revision;
    // Poll after the application's normal owner-thread pump or another import.
    host.pump()?;
    let next = host.combat_snapshot(Some(revision))?;
}
# Ok(())
# }
```

Only the latest ended encounter is retained for this API. Active-combat queries,
archives, callbacks and durable storage are not implemented. A later empty import
does not replace an earlier result. A failed import can replace it with partial
history. Completion status does not establish that the plugin accepted every raw
record or flushed its private deferred effects.
Explicit completion also does not flush the original writer or plugin-private
deferred effects. File delivery resumes after the boundary, including after
failure; use this command before importing while live combat is active.

## Values and ownership

| Scope | Data |
| --- | --- |
| Encounter | Zone, title, character, original start/end ticks and kind, signed duration ticks, allied damage/healing totals |
| Every combatant | Name, ally membership, damage/healing given/taken, duration, DPS/encounter DPS/HPS, hit percentage, hit/critical/heal/miss/block/kill/death counters |
| Ownership | Copied managed values at completion; Rust strings and vectors survive imports and host stop |
| Numeric fidelity | Signed ACT counters, NaN, infinity and negative zero are preserved; no Rust recalculation or clamping |
| Time fidelity | 100 ns ticks since year 1 and original `DateTimeKind`; graph-local timestamps have no inferred UTC offset |

Publication follows the combat-end callbacks. The result is frozen so later
plugin edits to the mutable ACT object graph cannot silently change the same
revision. Import completion changes only the frozen result's status and version.
A copy failure is retained for query instead of interrupting parsing. Managed
query callers receive a separate actor array as well.
Strings preserve plugin values. The pinned plugin's Linux resource parser can
retain a CR suffix in territory names; see [resource portability](m4-resource-portability.md).

## Private ABI

`ReadCombatSnapshot` uses the standard native delegate and a 64-byte header.
Requests and results are little endian. Output fields must initially be zero.
All capacities are validated before querying; insufficient capacity returns 1
without consuming a result. Success returns 0; caught errors return -1 and the
ordinary host diagnostic. This is private implementation detail.

| Offset | Field |
| --- | --- |
| 0, 4 | u32 capacity, version 1 |
| 8, 16 | u64 session, requested previous revision (0 for latest) |
| 24 | u64 returned revision, 0 for no result |
| 32, 36 | u32 actor count, flags: present bit 0, import bit 1, status in remaining bits |
| 40, 44 | u32 payload used, required total capacity (only for retry) |
| 48, 56 | u64 superseded versions, reserved zero |

The payload contains an 80-byte encounter header, contiguous UTF-8 zone/title/
character strings and actor records with 120-byte headers plus UTF-8 names.
Embedded NULs in ACT strings are preserved. Each actor has checked length/offset
fields and reserved zeros. Rust rejects malformed UTF-8, timestamps, flags,
versions, offsets, counts and unexpected trailing payload bytes before returning
owned values. Limits are 65,536 actors and 64 MiB of encoded payload.

## Verification boundary

Checks cover completed and partial imports, polling/coalescing, ownership,
malformed ABI data, capacity retries and snapshot failure recovery. The original
FFXIV synthetic fixture checks a 400-damage, two-second, two-combatant result with
the actual plugin. It is a controlled parser/transport fixture, not representative
gameplay, native capture, throughput evidence or FF Logs acceptance.
