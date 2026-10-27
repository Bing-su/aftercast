# Plugin-defined table columns

## ACT reference and current gap

The pinned ACT 3.8.5.288 binary (SHA-256
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`)
uses registry keys to reconcile five checked/reorderable column lists in
`FormActMain.ValidateTableSetup`. It appends newly registered keys using
DefaultVisible, preserves surviving checked/order entries, and removes keys no
longer registered. Duplicate display labels do not identify the same column.
Virtual row retrieval invokes GetCellData and optional cell colors on the UI owner
thread, catches cell exceptions and displays a column error. Combatant, attack and
action definitions also provide SortComparer callbacks; encounter and category
column definitions do not expose that callback.

| Row type / catalog level  | Native table context                 | Existing aftercast state                                                     |
| ------------------------- | ------------------------------------ | ---------------------------------------------------------------------------- |
| EncounterData / Encounter | Zone selected, encounter rows        | ColumnDefs retained; GUI encounter list uses fixed summary fields            |
| CombatantData / Combatant | Encounter selected, participant rows | ColumnDefs retained; typed Rust/Chorale participant table uses fixed fields  |
| DamageTypeData / Category | Combatant selected, category rows    | Category registrations retained; arbitrary formatted cells not queried       |
| AttackType / Attack       | Category selected, attack rows       | Typed aggregate queries exist; arbitrary formatted cells not queried         |
| MasterSwing / Action      | Attack selected, individual rows     | Canonical action/source queries exist; arbitrary formatted cells not queried |

## Managed catalog foundation

HostEnvironment.ReadTableColumns(TableLevel) runs on the host owner thread and
returns caller-owned arrays. It does not execute any formatter, SQL, color or sort
callback. Metadata includes registration key, display label, default visibility,
SQL metadata and available read/sort callbacks. Incoming/outgoing categories carry
separate keys, labels, ally values and colors; identical labels are not merged.

| Boundary          | Contract                                                                                                                                                                                                               |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Revision          | Per-host opaque monotonic value. Registry replacement, definition replacement, callback identity changes, registration order and category metadata changes invalidate it. It is not durable or cross-session identity. |
| Mutation          | Read actual registries on each query rather than trusting the last ValidateTableSetup cache. Return arrays cannot alter the cache. Invalid reads publish no new revision.                                              |
| Limits            | At most 256 columns per level, 256 categories per direction and 1024 strict UTF-8 bytes per metadata string; reject null definitions, null registries, empty keys, NUL text and invalid surrogate sequences.           |
| Missing callbacks | CanRead/CanSort explicitly describe missing delegates. Metadata discovery itself remains safe when a plugin's formatter or comparer would throw.                                                                       |
| Ownership         | Owner-thread access only. Foreign unsynchronized writes to public ACT dictionaries remain unsupported; these public dictionaries are trusted plugin data, not a security boundary.                                     |

Synthetic IActPluginV1 lifecycle tests cover five levels, default flags and
registration order, equal labels with distinct keys/directions, callback and
registry replacement, removal/re-registration, caller array mutation, malformed
metadata/limits, wrong-thread rejection and clean host restart. These are managed
catalog tests, not native plugin UI or callback-value execution tests.

## Finite completion units

| Unit                                     | Status / completion condition                                                                                                                                                                                                                                                |
| ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C1 Catalog                               | Implemented managed foundation; Bridge/Rust access remains in C3.                                                                                                                                                                                                            |
| C2 Values and sort                       | Implemented managed scoped cells/colors and bounded stable sort with callback errors, registration invalidation and scope-membership checks; see C2 contract below. Cross-worker session ownership belongs to C3.                                                            |
| C3 Rust transport and GUI                | Pending: session-owned metadata/value API, existing Rust/UI/Chorale table integration, checked columns and reordering by stable registry key, supported primary/secondary sort, explicit unavailable cells. Reconcile additions/removals using the reference behavior above. |
| C4 Preferences and end-to-end regression | Pending: atomic local configuration save/reload for column order/selection/sort; preserve drafts on failure, reject stale replies during plugin/session changes; use synthetic plugin/controller tests and final whole checks. Native clicks remain Windows QA.              |

Managed C1/C2 delivery does not complete plugin-column support. Existing typed
aggregate/action APIs remain the current displayed data until C3–C4 are connected.

## C2 cell, color and sort contract

HostEnvironment.ReadTable(TableQuery) reads original ACT graph objects on the host
owner thread. The requested catalog revision must match before callbacks run.
The scope chooses one of the five row levels, using existing encounter IDs and
actor/category/attack keys. Actor display names also use ACT's uppercase lookup.
Unknown retained scopes return null; a valid empty scope returns an empty page.
Encounter rows reuse individual-encounter enumeration, without duplicating zone
aggregate rows. Row Index is an ordinal in this scope snapshot, not a durable
identifier or a source-log link. Existing action/source APIs retain that role.

| Boundary          | C2 behavior                                                                                                                                                                                                                                                                          |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Input             | Clone selected column keys before callbacks; require 1–32 distinct registered keys, offset >= 0, limit 1–256 and nonzero expected revision. Invalid scopes/sort columns fail before callbacks.                                                                                       |
| Rows              | At most 16,384 rows in a scope; reject larger scopes explicitly. Sort the whole bounded scope before paging. Missing columns with no comparer cannot be sorted.                                                                                                                      |
| Cell values       | GetCellData and foreground/background delegates receive original objects. SQL callbacks are not invoked. Transparent/Empty colors inherit; other ARGB values are preserved without native theme transforms.                                                                          |
| Invalid cell      | Exceptions, missing delegates, null/NUL/invalid UTF-8 or text above 4,096 UTF-8 bytes produce a column-error cell. Valid neighbors remain usable. Diagnose once per column per query.                                                                                                |
| Response          | At most 8 MiB of cell text. Exceeding the aggregate budget aborts the response; it is not silently truncated. Metadata/scope text retains C1 limits.                                                                                                                                 |
| Registry changes  | Check the selected binding before and after every callback. A removed/replaced binding aborts immediately and invalidates its catalog cache. Recheck the complete catalog before publishing to detect unrelated changes too.                                                         |
| Reentry/mutation  | Reject recursive table queries and combat mutations through root admission APIs during callbacks. Recheck scope membership/order by object reference before publishing.                                                                                                              |
| Stable order      | Use plugin primary/optional secondary comparison, normalized signs including int.MinValue, requested direction and original ordinal ties. A sort exception or budget failure returns the original scope order and an explicit SortError.                                             |
| Comparison checks | Self, antisymmetry, repeated observed pairs, observed order and tie-class contradictions are checked. Tables <=64 rows check all pairs; larger tables check observed pairs and adjacent results. Bound the composed comparison evaluations at 500,000 and observed pairs at 262,144. |

These checks do not prove arbitrary plugin comparer purity or unobserved large-table
transitivity. Comparison validation makes additional callback calls, so comparers
must be consistent and free of side effects. Plugins can still mutate public ACT
object internals without admission; arbitrary in-place mutation that leaves scope
membership unchanged is unsupported. This is not an immutable graph transaction.

No callback preemption or execution-time bound is introduced. A formatter/native
callback that never returns can block this managed call. Existing process isolation
and shutdown deadlines can retire an unresponsive worker once this runs through the
Host boundary; direct HostEnvironment calls have no process-isolation guarantee.
No live/native-hang plugin is executed by these tests.

Synthetic managed tests cover five levels, original identity/owner thread, cell and
color failures, no SQL callbacks, alias-safe inputs/results, recursive queries,
blocked admitted mutations, registration changes, stale revision/scope membership,
stable/secondary/descending sorts, inconsistent comparers, actual comparison budget
fallback, row limits and aggregate output limits. C3 must reserve bounded reply
space before callbacks; do not rerun formatting/sorting merely to probe capacity.
Session-owning Rust scope/revision wrappers and actual worker tests remain C3 work.
