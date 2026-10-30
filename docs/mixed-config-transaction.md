# Mixed Config publication core

This is a managed storage/design checkpoint, not completed mixed Config import.
Existing single-section importers still reject mixed Trigger/Spell sections. `HostEnvironment.PreviewMixedConfig` now validates both allowlisted sections and returns their existing previews without mutation; it does not admit or save a mixed import. The two revisions are observed independently and must both be revalidated under the future publication gates. Existing homogeneous
imports and the two XML file formats are unchanged. Original ACT Config serializers
and the allowlist remain documented in [Config batches](config-batch.md): ACT may
continue after per-section failures, whereas aftercast intends all-or-none admission.

## Chosen boundary

`MixedConfigFiles` handles only `CustomTriggers.xml` and `SpellTimers.xml` in the
host data directory. Before either target changes, it flushes an atomically
published `.mixed-config.json` containing bounded before/after byte images.
It then atomically replaces each existing-format target and deletes the journal
at the commit point. An ordinary pre-commit failure rolls both targets back;
failed recovery retains the journal and reports the original and recovery errors.
Before loading either registry, HostEnvironment rolls back any unfinished journal.
A process exit after either file replacement therefore recovers the prior pair
before plugin initialization. Recovery is idempotent, including midway rollback.

A shared file lease excludes cooperating transactions. Expected before-images
reject stale previews/duplicate publication. Recovery accepts only exact before
or after bytes; an unrelated external edit causes a visible conflict and no
attempt to overwrite that edited content. Checks are not a lock against a
noncooperating writer between check and rename. No security boundary against
malicious paths, symlinks or journal tampering is claimed.

Each definition image is at most 4 MiB. Journal files are bounded to 24 MiB;
file stages are flushed, but directories are not synced. Synthetic child exits
exercise **process interruption with an operating filesystem**, not OS crash,
power loss, disk failure, or universal filesystem rename guarantees. The journal
is local rollback material, not a new archive/backup policy.

## Remaining managed integration before enabling mixed input

1. Mixed preview enforces the total 16 KiB/32-item budget and compiles both definitions. The future import must revalidate both revisions and reject all conflicts under the publication gates.
2. Suspend parsing and obtain both definition gates in one fixed order. Snapshot
   both registry revisions and both file before-images. Existing standalone save
   paths must participate in the same serialization boundary; do not enable a
   mixed caller while another host-managed save can race its file publication.
3. Reuse existing XML serializers to prepare and validate the complete resulting
   pair. Prebuild registry updates without changing active timer frames or firing
   sound/TTS/trigger effects. The file core accepts bytes, not validated ACT XML.
4. Commit the file pair, then publish prepared memory state without user callbacks.
   A post-commit publication failure must retire the session and restore both
   registries from the committed pair; continuing with one old registry is unsafe.
   Catastrophic allocation/process failure is handled by restart, not rollback
   promises about arbitrary plugin references or native callbacks.
5. Only then add a combined revision/session-owned preview/import operation to
   HostEnvironment/Bridge/Rust and the existing Trigger/Timer UI. A stale request,
   duplicate, canceled preview or failure before admission must change nothing.
   Once commit is admitted, cancellation cannot pretend completed writes vanished.

The current tests cover injected failures before journal publication, after journal
publication, after each target replacement and before commit; actual child-process
exit/restart recovery at all journaled boundaries; external conflict preservation,
cooperating lease exclusion and stale before-image rejection. Memory publication,
combined XML admission, mixed stale-session handling and Bridge/Rust/UI are not
implemented or verified in this checkpoint. Independent review remains quota-pending.
