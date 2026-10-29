# Local mini parse preview

Pinned ACT 3.8.5.288 IL (`test:original`, report-export-policy) has a
one-second `tmrTick` and refreshes its visible mini window at a configured
1–60 tick interval, default 5. `UpdateMiniEnc` reads the active zone's current
encounter and uses the selected registered text format or ACT's default.
`EndCombat` calls `UpdateMiniEnc` when the window is visible; the ordinary tick
skips mini refresh while out of combat. The original thus refreshes its final
frame at combat end and retains it while idle. These are static IL checks, not
native ACT execution.

The analysis panel and an optional singleton nonmodal Dioxus window share one
local preview and at most one accepted request per session. Repeated Open focuses
the existing window. The window continues through main-page navigation, closes
on host/session invalidation, and may be reopened. Closing the last view stops
refreshing. A pending creation or result is generation-checked before use; an
accepted in-flight request must finish before another is admitted in that session.
The isolated worker still cannot preempt arbitrary native plugin hangs.

The current-zone encounter endpoint excludes imports and incomplete transitions.
It retains the last ended encounter, rather than treating idle as an empty
result. After combat finalization and end subscribers, the owner publishes its
combat snapshot revision; the worker pump sends a coalesced, session-owned change
notification to the mini controller. It requests a final frame without waiting for
the configured periodic interval. If an older request is in flight, its response is
retired and one refresh follows; repeated notifications coalesce. The current-zone
identity and status are checked before and after rendering, so a new combat or zone
takes precedence over an older final frame. Idle polls skip formatting until an
encounter, settings or registered formatter catalog changes. Notification delivery
is subject to worker pump, busy operations and callback latency: this is not a
claim of identical native ACT timing.
No current encounter retains the previous frame, if any. A new encounter changes
the identity and is rendered; both pre- and post-render identity/status are
checked to discard stale results.

Both views reuse the existing registered formatter/sort callbacks, catalog
revision and bounded report path, with ACT's default or a saved local text
profile. The interval and profile name persist in app settings; starting is not
persisted. Missing profiles or their missing sort registration are shown as
errors. Unregistered formatter variables keep the existing ACT-compatible token
behavior. Plugin presets can be copied to a local profile in the report editor.

Output is escaped text with an explicit 64 KiB preview limit; empty text and no
current encounter are distinct. No clipboard, network, audio or game input is
used by this preview. ACT's native mini window, click-through, opacity, graph and
native font alignment are not reproduced. No native window clicks or Windows
execution have been validated.

ACT’s native graph branch passes the selected format’s sorting key and the
mini picture-box dimensions to `GenEncounterGraph`; its aligned text branch
passes the native control font to the `GetTextExport` overload. The existing
`GraphNumeric`/`GraphSvg` path serves static hierarchy reports and is not yet
wired to this window or validated as the native mini graph. Font measurement,
click-through and opacity remain separate native compatibility work.
