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
used by this preview. ACT's native mini window, click-through, opacity and native font alignment are not reproduced. No native window clicks or Windows
execution have been validated.

ACT’s native graph branch passes the selected format’s sorting key and the
mini picture-box dimensions to `GenEncounterGraph`; its aligned text branch
passes the native control font to the `GetTextExport` overload. The app can now
select a local Simple graph instead of text. The same registered sorting key and
SQL callback, owner-thread guard, selective export filter and Simple numeric
calculation used by hierarchy reports are reused. A local combatant-only capture
does not replace a user's separate hierarchy report capture. The current-zone
encounter identity/status is checked before and after the request, and the normal
mini ticket prevents a late frame entering a replaced session or closed window.
The selected key disappearing or a sort callback failure is an explicit error.
Per-row SQL failures follow the existing ACT-derived Simple contract and are skipped.

The graph mode is saved with the mini preferences and defaults to text for existing
settings; that migration default is an aftercast choice, not a verified native ACT
default. The existing global Simple graph ally and totals options are reused and
saved with graph preferences. Positive finite values, ACT order, axis, total and
average are computed by `GraphNumeric.Simple`; zero and negative values are not
plotted, no actors and no positive values are distinct, and nonfinite geometry is
rejected explicitly. The UI builds escaped text and bounded horizontal bars using ACT default DarkRed/DarkBlue alternation and DarkViolet for an exact case-sensitive character-name match,
never runs graph HTML, and displays at most 512 rows with the omitted count; the
axis and totals still include all accepted rows. Labels use the owner culture and ACT default suffix/decimal settings, with ACT's distinct tooltip and total/average formats. Graph palette, graph surface colors and suffix/decimal settings persist in the app and apply to the embedded and independent mini preview; graph-specific settings do not reuse plugin cell or ally/self table colors. Background, legend, outline, grid, internal text and total/average text use separate original-derived roles. The total/average role applies ACT's fixed 100/255 opacity; other settings use opaque #RRGGBB in this UI. Editing a surface color immediately publishes the draft to an open mini window; invalid drafts fall back safely for rendering and cannot be saved. SystemColors use explicit cross-platform fallbacks, and horizontal layout, text-placement decisions, native color alpha, dimensions and pixel alignment remain different. A capture is limited to
16,384 rows, two seconds, 4,096 UTF-16 units / 8 KiB UTF-8 per label and a conservative 3 MiB JSON-expansion budget; the worker reply limit is 4 MiB. Callbacks already running cannot be preempted.

Synthetic worker/controller tests cover numeric axes/order/totals, zero/negative,
nonfinite input, row/sort callback failure, missing keys, empty combat, final frame,
new encounter, session/window invalidation and preservation of an independent
hierarchy report capture. The checked-in original-derived Simple numeric oracle
verifies arithmetic, not native GDI execution. Native window clicks, native ACT
scheduling, WinForms drawing, font measurement, click-through, opacity and Windows
execution remain unverified.

ACT `GenEncounterGraph` uses graph-specific ColorControls, not plugin cell colors, ally text color or personal table background; it selects self by ordinal exact `ActGlobals.charName` equality before alternating colors. These conclusions are from local original decompilation; native GDI rendering has not been executed. Label/axis values outside Int64 range are rejected explicitly, rather than relying on platform-dependent conversion.
