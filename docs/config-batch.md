# Local ACT Config batches

The pinned ACT `SaveNewSettings` writes a `Config` containing `CustomTriggers`,
`SpellTimers` and unrelated application sections; shared Config snippets embed
that XML in the `Xml` attribute. ACT loads sections and may update existing keys
or continue after individual failures. Aftercast accepts either the embedded form
or a direct `Config`, but only if it contains exactly one `CustomTriggers` or one
`SpellTimers` section. It rejects mixed sections, unknown settings and attributes,
DTDs, external entities, empty lists, more than 32 entries and input over 16 KiB.
This allowlist avoids applying paths, plugins, subscriptions or other settings.
The attributes follow ACT's full Config format rather than the short Trigger/Spell
snippet format. Malformed booleans/numbers and editor-invalid values are errors,
instead of ACT's partial fallback behavior.

Preview validates every definition, returns each key, conflicts and the owning
session/revision, and changes no registry or file. Import adds all entries to a
single existing definitions file using its atomic replacement; it checks revision,
within-batch duplicates and existing keys before writing. It never overwrites an
existing key or invokes sound, speech or timer execution. Cross-file all-or-none
publication is not provided; mixed Trigger/Spell Config is rejected, not split.
No network, clipboard or subscriptions are used. Synthetic managed and isolated
worker tests cover both types, malformed/mixed input, conflicts, stale revisions,
write failure, effect isolation and restart. UI integration is not yet complete.
