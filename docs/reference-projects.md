# Reference projects

Research snapshot: 2026-10-20. This investigation originally assumed an independent
Rust FFXIV parser without ACT/.NET. That premise is superseded by the approved
[ACT-host blueprint](blueprint.md). These are reference boundaries, not selected
runtime dependencies or proof of game/platform compatibility.

| Project | Role | Useful boundary / limit | Repository license snapshot |
| --- | --- | --- | --- |
| [cactbot](https://github.com/OverlayPlugin/cactbot) | Log consumer and raid overlay | Formats, fields, tests; not a capture or standalone DPS engine | Apache-2.0 |
| [OverlayPlugin](https://github.com/OverlayPlugin/OverlayPlugin) | ACT-to-browser API/events | WebSocket, handlers, CombatData; full cactbot requires more than CombatData | MIT plus component notices |
| [Machina](https://github.com/ravahn/machina) | Capture and FFXIV stream processing | TCP reconstruction, compression/framing, Deucalion integration; C# | GPL-3.0 |
| [Unscrambler](https://github.com/perchbirdd/Unscrambler) | Packet deobfuscation | Version-specific decoding, not capture/aggregation | WTFPL |
| [Deucalion](https://github.com/ff14wed/deucalion) | In-process packet capture | Rust Windows injection DLL; Rust does not imply native Linux support | GPL-3.0 |
| [TemporalStasis](https://github.com/NotNite/TemporalStasis) | Network proxy | Requires Oodle; excludes deobfuscation and combat opcodes/structs | MIT |
| [IINACT](https://github.com/marzent/IINACT) | Dalamud host for existing parser | ACT replacement boundaries; not an independent Rust parser | GPL-3.0 |
| [FFXIV ACT Plugin](https://github.com/ravahn/FFXIV_ACT_Plugin) | Capture/parser baseline | Public API and outputs; repository does not provide the private parser implementation | No established source-reuse license |
| [Ember](https://github.com/GoldenChrysus/ffxiv-ember-overlay) | Browser meter/timers | Candidate real consumer; does not produce parser data | GPL-3.0 |

Check actual files and bundled dependencies before copying or redistribution.
Sources: [cactbot license](https://github.com/OverlayPlugin/cactbot/blob/main/LICENSE),
[OverlayPlugin notices](https://github.com/OverlayPlugin/OverlayPlugin/blob/main/LICENSE.txt),
[Ember license](https://github.com/GoldenChrysus/ffxiv-ember-overlay/blob/bleeding-edge/LICENSE),
and each upstream repository's README/license.

## Reading map

| Source | What to verify |
| --- | --- |
| [cactbot LogGuide](https://github.com/OverlayPlugin/cactbot/blob/main/docs/LogGuide.md) | Network_*.log is already plugin-processed text, not raw TCP; entity IDs are not permanent global identities |
| [netlog_defs.ts](https://github.com/OverlayPlugin/cactbot/blob/main/resources/netlog_defs.ts) | Log types and field positions |
| [cactbot tests](https://github.com/OverlayPlugin/cactbot/tree/main/test) | Consumer parsing regressions |
| [Overlay developer API](https://overlayplugin.github.io/OverlayPlugin/devs/) | addOverlayListener, callOverlayHandler, startOverlayEvents, OVERLAY_WS |
| [Event types](https://overlayplugin.github.io/OverlayPlugin/devs/event_types) | CombatData, LogLine, ChangeZone, ChangePrimaryPlayer |
| [MiniParse source](https://github.com/OverlayPlugin/OverlayPlugin/blob/main/OverlayPlugin.Core/EventSources/MiniParseEventSource.cs) | String-valued Encounter/Combatant dictionaries and isActive; do not casually replace values with numbers/bools |
| [cactbot plugin](https://github.com/OverlayPlugin/cactbot/tree/main/plugin) | Extra event sources/handlers beyond a meter |
| [Unscrambler explanation](https://github.com/perchbirdd/Unscrambler#what-is-this) | Version-changing obfuscation since 7.2 |
| [ACT repository](https://github.com/EQAditu/AdvancedCombatTracker) | Plugins/distributions/translations/issues; not evidence that complete ACT core source is public |

Capture, reconstruction, decompression, deobfuscation, event decoding, actor state,
combat semantics, and aggregation are distinct tasks. A proxy is active connection
mediation, not passive packet copying. File import cannot prove live capture.
IINACT's examined README describes Unscrambler without additional Deucalion
injection; older descriptions should not be treated as its current architecture.

## Maintenance snapshot

GitHub default-branch/latest-release observations below were recorded on
2026-10-20 (UTC dates). Latest release means the API result, not every distribution
channel or proof of compatibility with every region.

| Project | Branch commit date | Latest stable release observed |
| --- | --- | --- |
| FFXIV ACT Plugin | 2026-10-13 | 3.0.3.3, 2026-10-13 |
| OverlayPlugin | 2026-10-13 | v0.19.110, 2026-10-13 |
| cactbot | 2026-10-08 | v0.37.5, 2026-08-10 |
| IINACT | 2026-10-13 | v2.10.3.10, 2026-10-13 |
| Deucalion | 2026-10-13 | 1.5.1, 2026-10-13 |
| Machina | 2026-10-13 | 2.3.1.1, 2022-07-06; branch activity mentions 2.4.8.1/7.57 |
| TemporalStasis | 2026-02-18 | No stable release returned; net10 update does not prove combat decoding |
| Unscrambler | 2026-10-13 | No stable release returned; branch mentions 7.57 |
| Ember | 2026-09-28 | 1.10.0, 2026-09-26 |

Use OverlayPlugin/cactbot rather than the old quisquous/cactbot location. The
original suggestions were saved-log comparisons, capture experiments, a real
overlay consumer, and independent FF Logs upload validation. Only the comparison
and consumer requirements carry forward; independent capture/parser is no longer
the default product design.

No candidate was run in-game by this research. DPS/HPS, pets, DoT/HoT, overheal,
Windows/Linux capture, overlays, FF Logs acceptance, and complete transitive
redistribution terms were not validated. See [SDK research](ffxiv-act-sdk-analysis.md)
and [TemporalStasis research](temporalstasis-analysis.md) for the later evidence.
