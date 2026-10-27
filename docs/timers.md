# Spell timer definitions and transient countdowns

The managed definition service implements ACT 3.8.5.288 spell-timer data and
valid `SpellTimers/Spell` XML attributes, with session-owned Rust management
operations and the Spell timers definition editor. A definition's `ActiveInList` flag controls new matching;
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

## Session-owned Rust API and editor

| Surface                            | Contract                                                                                                                                                                                                                                                                                               |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `timer_definitions(offset, limit)` | Returns owned snapshots, an opaque session-owned `TimerRevision` and retained frame count. Limits are 1..16.                                                                                                                                                                                           |
| `save_timer_definition`            | Returns `SavedTimer` with the authoritative managed key and normalized definition. Rust does not reconstruct ACT's culture-dependent keys.                                                                                                                                                             |
| Delete/reload                      | Require the expected revision. Old-session revisions are rejected before worker dispatch. The host owns the `SpellTimers.xml` path; callers cannot provide an arbitrary persistence path.                                                                                                              |
| Worker                             | Runs requests on the original owner context and quiesces log/action processing for mutations. Fixed ABI capacity and input are validated before writes.                                                                                                                                                |
| Editor                             | Uses existing Rust/UI Input, Label, Button and Accordion. All 18 fields remain editable; numeric/color text is preserved even when invalid. ARGB uses #AARRGGBB including alpha. Saving never previews sound.                                                                                          |
| Draft and conflicts                | Navigation/refresh/paging preserves dirty drafts. Selecting another row cannot discard one. A changed revision requires reviewing all saved fields before an explicit rebase, or keeping a new copy; duplicate category/name creation is rejected. Discard resets the form; Stop clears session state. |
| Pending work                       | One request is admitted at a time; mutations block conflicting host operations. App-owned tasks survive pane unmount. Old-session replies cannot update newer drafts, clear newer busy flags or surface stale errors.                                                                                  |
| Acknowledgment                     | Apply the managed key/definition before a follow-up read. A failed read does not turn an acknowledged save into an unsaved draft or cause an automatic retry. Write failures preserve the complete draft.                                                                                              |
| Deletion                           | Confirm the rendered key/revision explicitly. Editing cancels pending confirmation. Deleting definitions does not cancel retained countdowns.                                                                                                                                                          |

Synthetic worker and GUI-controller tests cover persistence, failed replacement,
normalized identity, stale revisions/sessions, draft navigation, duplicates,
reviewed conflicts, deletion, late successful replies, multiple pages and last-page
fallback. The Windows desktop scenario is registered but has not run on Linux.

Transient timer inspection/manual start/cancel and native spell-window input/audio
remain separate timer surfaces; this editor reports the retained frame count only.
Actual desktop clicks and genuine native timer-model execution remain Windows
validation. The partial WinForms rendering bridge remains planning only.
