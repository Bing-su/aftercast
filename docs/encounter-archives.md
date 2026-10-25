# Encounter archives and history

The host imports/exports ACT encounter XML and single-member gzip `.act` files,
and explicitly checkpoints completed encounters to its private history database.
Raw log import remains a separate parser workflow.

| Rust API                                | Result and contract                                                                             |
| --------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `import_act(path)`                      | Number of encounters added; recognizes XML or gzip by content. Malformed input adds none        |
| `export_act(id, path, ActFormat::Xml)`  | Write one retained encounter to a **new** XML file                                              |
| `export_act(id, path, ActFormat::Gzip)` | Write interchange XML inside a `.act` gzip member; an existing destination is preserved         |
| `save_history()`                        | Checkpoint retained completed encounters and return the total stored count                      |
| `restore_history(filter)`               | Restore matching saved encounters and return the newly restored count; empty filter matches all |

Initialize the same parser's category registrations before importing or restoring.
Operations require the owner thread, outside dispatched callbacks, with live
combat ended. They suspend file delivery and action admission while working.
Reconstruction reads saved swings directly: raw parser hooks, correction rules,
live timers and audio are not replayed.

```rust,ignore
// Initialize the parser first so saved swings have their original category definitions.
host.import_act("encounter.act")?;
let encounters = host.encounters(0, 16)?;
if let Some(encounter) = encounters.items.first() {
    host.export_act(encounter.id, "copy.act", aftercast::ActFormat::Gzip)?;
}
host.save_history()?;
host.stop()?;

// After starting another Host with the same data directory and initializing its parser:
let restored = restarted.restore_history("Zeromus")?;
let page = restarted.encounters(0, 16)?;
```

## Database ownership and recovery

| Concern                | Behavior                                                                                                                                                                   |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Checkpoint             | Explicit; neither shutdown nor each combat end automatically checkpoints. Recovery reaches the last successful `save_history()`                                            |
| Storage                | `history/index.xml` is a metadata index; immutable SHA-256-named `.act` files hold encounter actions in file-backed ACT-style storage                                      |
| Commit                 | Flush payloads first, then flush and atomically replace the index. A failed write preserves the previous committed index; preceding payload writes may remain unreferenced |
| Concurrent checkpoints | An exclusive `history/checkpoint.lock` lease rejects competing writers. The latest committed index is merged under that lease                                              |
| Startup                | Load metadata only. Restore after parser initialization; arbitrary callbacks/types are never restored from disk                                                            |
| Search                 | Case-insensitive substring match on title, zone or character; restoration refreshes the committed index first                                                              |
| Duplicate restoration  | Already restored or checkpointed payloads are skipped in the current graph. Identical serialized contents share a stored payload                                           |
| Identity               | Restored encounters receive fresh session IDs; old `EncounterId` values remain invalid after restart                                                                       |
| Integrity              | Validate archive filenames and SHA-256 before restoration; a corrupt selected file publishes none of that call's encounters                                                |
| Precision              | Private payloads retain subsecond timestamps, DateTime kind, underlying Dnum number/label/suffix, swing tags, saved log lines, segment times and allies                    |
| Retention              | Superseded/unreferenced payloads are not deleted automatically; retention/compaction remains future work                                                                   |

ACT interchange retains ACT's second-resolution timestamps and display-form damage,
which can lose the underlying numeric value of a custom damage label. Private
payloads add numeric attributes and round-trip timestamps to avoid that loss.
Category/column registrations and arbitrary combatant/encounter/category tags are
not serialized; derived totals depend on the initialized parser's definitions.

The source folder hint is captured before raw-import context restoration. History
restoration reads saved swings; it does not locate byte ranges in the original log
or run the original parser again. Original-log subscriptions and durable recording
remain separate contracts.

## ACT-facing contracts

| Contract                                             | Scope                                                                                                                             |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `EncountersToXml(List<EncounterData>, Stream, bool)` | Encounters, segments, outgoing swings, DataContractSerializer swing tags, optional saved log lines and allies; leaves output open |
| `XmlToEncounters(Stream)`                            | Detached imported encounters; closes input as original ACT does                                                                   |
| `MemCompressGZip` / `MemDecompressGzip`              | Single-member gzip; replacement also verifies footer CRC/size and rejects truncation                                              |
| `ZoneDatabase` / `EncDatabase`                       | Original sorted metadata collections and independently compared replacement/exclusion policies                                    |
| `SaveXmlRecords` / `LoadXmlRecords`                  | ACT `Records/Record` XML with binary DateTime values; metadata alone does not contain actions                                     |

Files and expanded XML are limited to 64 MiB. DTDs/external resolution are disabled
and tag object graphs are bounded. Unregistered outgoing swing types, unknown
serialized tag types and invalid XML fail visibly without a successful partial
archive import.

## Verification

| Check                                                                                                                     | Evidence                                                                                                                        |
| ------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Original ACT 3.8.5.288 XML export/import, gzip and metadata policies                                                      | `mise run test:encounter-archives`; shared probes compile against ACT and execute in independent original/replacement processes |
| Checkpoint/restart/search/restore and precise values                                                                      | `EncounterArchiveChecks.cs`, included in `mise run managed:test`                                                                |
| Corrupt/truncated gzip, malformed XML/DTD/nil tags/index paths, unsupported tags, contention and destination preservation | Same managed checks, plus `ArchiveAbiChecks.cs` for native packet validation                                                    |
| Public Rust workflow and old-session ID rejection                                                                         | `crates/aftercast/tests/archives.rs`, included in `mise run test:plugins`                                                       |

Controlled Linux checks do not establish full ACT application workflows, every
historical format variant, Windows execution, power-loss survival of directory
metadata, or whole-database restoration performance on a large history.
