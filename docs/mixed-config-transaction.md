# Mixed Config publication core

The managed `HostEnvironment.PreviewMixedConfig` and `ImportMixedConfig` now validate
and publish a mixed Trigger/Spell Config. Existing single-section importers and
Bridge/Rust and the Trigger-pane mixed Config UI expose combined session-owned
preview and explicit import. Single-kind interfaces still reject mixed input. Both existing XML file formats remain unchanged.
The allowlist and original ACT serializers are documented in
[Config batches](config-batch.md). ACT can continue after per-section failures;
aftercast rejects the whole mixed admission on validation/conflict failure.

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

A shared file lease excludes cooperating transactions. The existing standalone
trigger and timer file writers now acquire that lease while their registry gate
is held and recover a pending journal before writing. Synthetic tests cover both
writers refusing a held lease without changing disk or memory. This does not prevent noncooperating external writers. Expected before-images
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

## Managed publication and fail-stop boundary

`ImportMixedConfig(text, triggerRevision, timerRevision)` revalidates the 16 KiB /
32-item allowlist, compiles both sections, suspends log reading/combat actions,
and locks trigger then timer registries. Both supplied revisions and existing-key
conflicts are checked before admission. It prepares complete XML images through
the unchanged serializers in a private staging directory, retaining existing timer
objects/countdown frames and existing trigger objects. Staging is removed on both
success and failure. The file before-images are captured before preparation and
rechecked under the shared file lease, so a detected external edit is never adopted
as permission to replace that file. Preview revisions describe registries, not the
historical identity of a file or a filesystem lock.

After the pair commit point, both prepared registries are published and lookup /
active indexes rebuilt under the same gates, without invoking sound/TTS/timer or
plugin callbacks. A failure after commit retires the host: parsing is stopped,
ACT globals are released, and Dispose **skips both definition saves**. A detected
external-file conflict or retained recovery journal also retires the host to prevent
normal Dispose overwriting an external edit or unresolved transaction. Cleanup
errors are aggregated with the original failure. The error does not mean committed
writes were rolled back. A fresh HostEnvironment loads both committed files (or
recovers an unfinished journal before loading either). There is no in-session
rollback promise for arbitrary plugin-held object references.

PluginSession rejects further owner-dispatched requests on a faulted environment
and marks the worker unsafe for reuse. The integration must terminate/recreate the
worker, rather than retrying plugin requests or DeInit callbacks against partial
memory; Stop cannot make that worker reusable. Ordinary shutdown and single-kind
saves keep their previous persistence behavior. Lease contention and stale registry
revisions before admission leave memory/files unchanged and allow a fresh retry.
Mixed import has no mid-commit cancellation: cancellation belongs before admission.

Synthetic managed tests cover successful mixed import/restart, duplicate retry,
stale revisions, existing keys, both standalone writers contending with mixed
publication, held file lease, external edit preservation through Dispose/restart,
file-stage rollback and memory failures immediately after commit / first registry /
second registry. The latter prove Dispose cannot overwrite the committed pair and
faulted session requests are rejected. Existing core tests separately exercise
real child-process exits and startup recovery at journaled file boundaries.
These are process-failure/contract checks, not native ACT UI or OS/power-loss tests.
Independent review remains quota-pending.

## UI and worker boundary

The existing Trigger pane uses the Rust preview object, which owns the exact XML,
both revisions and the worker session. Editing or canceling discards the preview;
conflicting entries disable import. A successful explicit import refreshes both
lists. Failure preserves the text draft and never retries automatically. A typed
post-commit publication fault and an uncertain transport/response failure retire
the affected worker even if its result arrives after the pane was edited or left;
a response from an older session cannot stop the replacement. Normal stale revision
and validation failures leave the worker available. The next session can preview
the preserved draft again.

Synthetic controller tests cover editing, cancellation, duplicate clicks, stale
timer revision, late results and session replacement. Isolated workers cover mixed
success, both stale revisions, immutable input and restart. A synthetic plugin uses
the existing private managed fault seam in a worker to exercise memory-publication
failure, request rejection and new-worker recovery. This is not a native ACT or
desktop click test; native Windows and GUI behavior remain unverified.
