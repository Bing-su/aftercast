# Spell timer definitions and transient countdowns

The managed definition foundation implements ACT 3.8.5.288 spell-timer data and
valid `SpellTimers/Spell` XML attributes. Rust transport and the definition editor
are not yet connected. A definition's `ActiveInList` flag controls new matching;
it is separate from already retained frames and running countdowns.

## Definition management

| Operation             | Contract                                                                                                                                                                                                                                                                                           |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| List                  | Up to 16 owned definition snapshots, content revision, total definition count and retained frame count. The latter includes frames waiting for expiry/removal and is not a count of enabled definitions.                                                                                           |
| Save                  | Create or replace using an expected content revision and optional previous key. Category/name determine the ACT key through the existing `TimerData.Key`; collisions use the registry's actual culture comparer, including equivalent Unicode spellings.                                           |
| Validation            | Editor name/category limits are 512 characters, all text fields at most 4,096 characters, no null/NUL. ACT editor ranges are duration 0..3600 seconds, warning 0..100 and removal -600..0. XML/plugin integer values outside editor ranges remain readable and are not clamped during persistence. |
| Atomic publication    | Serialize an owned candidate to a unique adjacent temporary file, flush, enforce the 4 MiB limit, fully validate the candidate and replace `SpellTimers.xml` before changing the registry. Ordinary write/serialization/validation failures preserve the prior file and runtime definitions.       |
| External plugin edits | Recompute snapshot values, including direct public `TimerData` field/property changes, before accepting a revision. Stale editors must refresh instead of overwriting unseen changes.                                                                                                              |
| Startup/shutdown      | Load before publishing ACT globals; save after stopping processing. Invalid startup files fail explicitly without rewriting the file or acquiring the globals. Empty saved definitions remain empty on restart. Countdown frames are never serialized.                                             |

The standalone private file writes all definition fields: enablement, name/category,
duration, master-tick flag, category/self restrictions, absolute timing, start/warning
cues, warning/removal values, radial/modifiable flags, tooltip, signed ARGB fill color
and both panel flags. Boolean reading accepts ACT's `True`/`False` as well as their
lowercase forms. Schema-style `1`/`0` is rejected, matching ACT `Boolean.Parse`.
DTD/external entity processing is prohibited. Temporary files belong only to the
current save attempt. This is not a journal or a process/OS-crash recovery promise.

## Behavior while timers exist

| Change         | Existing frame/countdown behavior                                                                                                                                                                                                                                                           |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Add/Edit       | Replace the registry entry without replacing existing frame references or restarting countdowns. Unrelated registry objects retain their identity.                                                                                                                                          |
| Next Notify    | Reuse the existing runtime: attach the selected new definition before checking absolute timing and the two-second gate. New countdowns use the new duration; an existing countdown retains its captured duration/start.                                                                     |
| Disable/delete | Prevent future matching through the definition registry; do not cancel already retained frames.                                                                                                                                                                                             |
| Private reload | Validate the complete file and every key before applying any changes. Reuse same-key registry objects so attached frames see restored warning/display metadata; preserve captured countdown durations. Remove registry entries absent from the private file without canceling their frames. |

Private reload is a strict replacement of the complete definition registry. ACT's
settings import instead merges entries and skips individual invalid attributes.
That partial-import behavior is not exposed by this private restore operation.
Frames already detached by an earlier definition replacement remain detached until
the runtime's normal Notify path reconnects them, matching ACT's object ownership.

## Evidence and remaining work

The source comparison used genuine ACT `FormSpellTimers.AddEditTimerDef`,
`RemoveTimerDef`, `NotifySpell`, `btnAddEdit_Click` and
`FormActMain.SaveXmlSpellTimers`/`LoadXmlSpellTimers`. Synthetic managed tests cover
all field round trips, XML escaping/boolean spellings, empty files, Unicode name
collisions/aliases, stale revisions, corrupt/DTD inputs, write failures, live-frame
identity, rebind order, retained duration, restart and failed-startup cleanup.

Next: session-owned Bridge/Rust operations, explicit definition editing/reload GUI,
input/draft/error state contracts and separate transient timer inspection. Reuse
existing Rust/UI components and runtime matching; do not restore live countdowns
from the definition file. Native spell-window input, audio and genuine timer-model
execution remain Windows validation. The partial WinForms rendering bridge remains
planning only.
