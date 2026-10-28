# Local ACT trigger snippets

ACT 3.8.5.288 `ShareCustomTriggerToXml` emits one `<Trigger>` element and
`ShareXmlToCustomTrigger` requires `R` (regex) and `C` (category). Optional
`SD`, `ST`, `CR`, `T`, `TN` and `Ta` supply sound, sound type, zone restriction,
timer, timer name and tabbed-result settings. ACT treats `T` as true for boolean
attributes; a missing or unparsable sound type leaves the new-trigger default.
The checked original serializer fixture is `CustomTriggerProbe`; parser behavior
is from original code inspection, not a native UI execution.

Aftercast accepts one explicitly entered Trigger element, with a 16 KiB UTF-8
limit. XML DTDs, external entities, unknown attributes, child elements, Spell and
Config snippets are rejected. The original serializer's numeric entities decode
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
remains committed. The existing ACT Spell share attributes have been inspected but
Spell import and multi-item XML are not implemented in this bounded path. Native
GUI clicks and Windows remain unverified.
