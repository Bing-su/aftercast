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

| Unit                                     | Status / completion condition                                                                                                                                                                                                                                                                                                         |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C1 Catalog                               | Implemented managed foundation; Bridge/Rust access remains in C3.                                                                                                                                                                                                                                                                     |
| C2 Values and sort                       | Pending: bounded scoped rows against original ACT graph objects; invoke captured plugin formatting/colors/comparison on owner thread, contain callback/null/invalid/oversized output errors, stable ties and explicit sort failures. Recheck registration revision when callbacks mutate registrations; reject stale scopes/sessions. |
| C3 Rust transport and GUI                | Pending: session-owned metadata/value API, existing Rust/UI/Chorale table integration, checked columns and reordering by stable registry key, supported primary/secondary sort, explicit unavailable cells. Reconcile additions/removals using the reference behavior above.                                                          |
| C4 Preferences and end-to-end regression | Pending: atomic local configuration save/reload for column order/selection/sort; preserve drafts on failure, reject stale replies during plugin/session changes; use synthetic plugin/controller tests and final whole checks. Native clicks remain Windows QA.                                                                       |

Catalog delivery alone does not complete plugin-column support. Existing typed
aggregate/action APIs remain the current displayed data until C2–C4 are connected.
