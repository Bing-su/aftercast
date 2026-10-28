# Local ACT trigger snippets

ACT 3.8.5.288 `ShareCustomTriggerToXml` emits one `<Trigger>` element and
`ShareXmlToCustomTrigger` requires `R` (regex) and `C` (category). Optional
`SD`, `ST`, `CR`, `T`, `TN` and `Ta` supply sound, sound type, zone restriction,
timer, timer name and tabbed-result settings. ACT treats `T` as true for boolean
attributes; a missing or unparsable sound type leaves the new-trigger default.
The checked original serializer fixture is `CustomTriggerProbe`; parser behavior
is from original code inspection, not a native UI execution.

Aftercast accepts one explicitly entered Trigger element, with a 16 KiB UTF-8
limit. XML DTDs, external entities, unknown attributes, child elements and Config snippets
are rejected. A Spell element is handled separately on the timer screen. The original serializer's numeric entities decode
normally. Existing editor text/range validation and compiled-regex timeout apply;
unsupported sound types fail explicitly. Preview captures the definition, ACT key,
conflict state and session-owned trigger revision without changing settings.
Only the user's explicit import writes the definition through the existing atomic
trigger save; it does not run sounds, TTS or timers. Duplicate keys are rejected,
whereas ACT's snippet handler modifies an existing matching trigger. A changed
revision requires a fresh preview, and the single-item transaction prevents partial
batch imports. A saved definition takes effect on future matching. Imported sound
and timer settings may take effect then, as for any other active trigger.

The Rust/UI panel accepts typed text only; no clipboard or subscription is read.
The preview displays the settings and refuses a conflicting item. Cancel clears
only the preview, while the text draft stays in the app session. A failure keeps
the preview; stop invalidates the session and a save acknowledged before navigation
remains committed. Homogeneous Config batches are supported separately; see [Config batches](config-batch.md).
Native GUI clicks and Windows remain unverified.

## Local Spell snippets

ACT `ShareXmlToSpell` requires `N` and `C`; `T`, `R`, `OM`, `A`, `WV`, `RD`,
`M`, `Tt`, `SS`, `WS`, `FC`, `RV` and `RC` override the new `TimerData`
defaults when present. Present boolean values equal to `T` are true; invalid
numeric values retain defaults. The pinned original method and defaults are
executed headlessly by `SpellSnippetProbe` and compared with synthetic managed
fixtures. Existing ACT definitions are modified by the original handler; the
aftercast preview instead marks their culture-aware key as conflicting and the
explicit import refuses to overwrite them.

The timer pane uses the same 16 KiB secure single-element XML parser and
session-owned preview/revision pattern as Trigger. The existing timer definition
editor requires a nonempty name and ranges duration 0–3600, warning 0–100 and
removal −600–0; a snippet outside those ranges is rejected rather than silently
clamped, a deliberate difference from ACT's raw snippet handler. Saving uses the
existing atomic definition file and never starts or resets a countdown or plays
a sound. Sound data in an imported definition can be used by a later timer event.
Homogeneous Config batches are supported separately; mixed or unrelated Config is rejected.
