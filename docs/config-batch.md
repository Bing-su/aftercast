# Local ACT Config batches

The pinned ACT `SaveNewSettings` writes a `Config` containing `CustomTriggers`,
`SpellTimers` and unrelated application sections; shared Config snippets embed
that XML in the `Xml` attribute. As in ACT, shared wrappers are rejected when
their decoded `Xml` contains the case-sensitive string `logFilePath`. ACT loads sections and may update existing keys
or continue after individual failures. The single-kind aftercast importers accept either the embedded form
or a direct `Config`, but only if it contains exactly one `CustomTriggers` or one
`SpellTimers` section. The separate mixed importer is described below. Single-kind endpoints reject mixed sections; all supported paths reject unknown settings and attributes,
DTDs, external entities, empty lists, more than 32 entries and input over 16 KiB.
This allowlist avoids applying paths, plugins, subscriptions or other settings.
The attributes follow ACT's full Config format rather than the short Trigger/Spell
snippet format. Malformed booleans/numbers and editor-invalid values are errors,
instead of ACT's partial fallback behavior.

Preview validates every definition, returns each key, conflicts and the owning
session/revision, and changes no registry or file. Import adds all entries to a
single existing definitions file using its atomic replacement; it checks revision,
within-batch duplicates and existing keys before writing. It never overwrites an
existing key or invokes sound, speech or timer execution. Single-kind importers reject mixed Trigger/Spell Config rather than splitting it;
the separate mixed importer uses the recoverable publication path described below.
No network, clipboard or subscriptions are used. Synthetic managed and isolated
worker tests cover both types, malformed/mixed input, conflicts, stale revisions,
write failure, effect isolation and restart. The existing Trigger and Timer panes
accept text, preview all entries and mark conflicts; explicit import adds the whole
batch. Controller tests cover cancellation, navigation and stale/session responses.
The pinned original `ConfigBatchProbe` executes its serializers to check attribute
names and values; native ACT UI and actual native desktop clicks are unverified.

The [mixed publication path](mixed-config-transaction.md) now supports managed
combined preview/import with bounded journaling, startup recovery, both registry
revisions, and fail-stop shutdown when committed memory publication fails. Existing
single-kind importers still reject mixed input. Bridge/Rust and the separate
Trigger-pane mixed Config panel provide session-owned preview and explicit import.
The restrictions above describe the single-kind interfaces. Synthetic managed publication tests prove both
registry admission and committed-pair preservation through failed publication and
restart. They do not prove OS crash/power-loss durability or native ACT UI parity.
