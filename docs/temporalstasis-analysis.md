# TemporalStasis and initial ACT binary research

Research date: 2026-10-20. TemporalStasis source is pinned to
40c0b9a7eaf53c79a0096b71ff3e7eec8c257476. This was an independent-capture
alternative; the approved [blueprint](blueprint.md) instead reuses the existing
FFXIV plugin. No proxy/collector from this investigation is adopted by default.

## Proxy boundary

| Step | Source observation | Implication |
| --- | --- | --- |
| Lobby | Example connects to DEV.LobbyHost04/LobbyPort04 through a local listener | Real region/DC startup must be configured |
| Upstream | LobbyProxy mediates a bidirectional TCP connection to the original server | Not a standalone fake localhost server |
| Lobby encryption | Serverbound EncryptionInit creates Brokefish state for IPC decoding | Distinct from zone packet obfuscation |
| EnterWorld | Saves original destination and rewrites host/port to local zone proxy | Transfer/reconnect mapping must stay correct |
| Zone/chat | One player opens separate zone/chat connections | A single connection is not a full session model |

Source: [TemporalStasis pinned tree](https://github.com/NotNite/TemporalStasis/tree/40c0b9a7eaf53c79a0096b71ff3e7eec8c257476).
A proxy does not remove the need for game-version decoding, state reconstruction,
and aggregation. It actively changes connection routing rather than passively
copying packet bytes.

## Framing and dependencies

| Layer | Size derived from source | Fields |
| --- | --- | --- |
| FrameHeader | 40 bytes | Prefix16, timestamp8, size4, connection type2, count2, version1, compression1, unknown2, decompressed size4 |
| SegmentHeader | 16 bytes | Size4, source actor4, target actor4, segment type2, unknown2 |
| IpcHeader | 16 bytes | Unknown2, opcode2, unknown2, server ID2, timestamp4, unknown4 |

Treat TCP as a stream: a read is not necessarily one complete frame. Validate
frame, segment and IPC boundaries against actual bytes, including decompression
output and size arithmetic, before interpretation.

OodleLibraryTcp calls native Oodle TCP functions. Connection lifetimes contain two
output states and separate directional input states, with state/shared/window
initialization and SetWindow/Train. Compression is not a stateless per-frame call.
The enum names None/Zlib/Oodle, but Zlib is explicitly excluded. Never reinterpret
an unsupported compression type as uncompressed data. The project excludes
obfuscation and combat opcode/structure definitions; decompressed IPC is not yet
a semantic combat event. See [upstream requirements](https://github.com/NotNite/TemporalStasis#requirements).

## Implementation issues not to copy

| Source pattern | Concrete risk / reconstruction requirement |
| --- | --- |
| ZoneProxy.nextServer overwritten by SetNextServer; example warns about multiple clients | Session destinations can race; validate connection mapping |
| Fixed 0x4000 buffer and unchecked external Size/Count/segment slices | Bounds errors or stale data interpretation; validate every nested boundary |
| Empty catch around event dispatch | Analysis failures disappear; separate forwarding from observer failure |
| Ignored EnterWorld rewrite exceptions | Subsequent connections may bypass capture without a visible error |
| Task.WhenAny without observing/finishing both directions | Fault/cancellation/cleanup ambiguity; join both tasks and report termination cause |
| SendPacketFrameAsync writes header before modifying local Size fields | Buffer header can remain stale; arbitrary packet transmission was excluded from the proposed first version |

These are source observations and expected consequences, not reproduced game
failures. No proxy was run, injected, or connected to a game.

## Initial ACT observations

| Identity | Value |
| --- | --- |
| SHA-256 | 934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657 |
| MVID | 94f61cd2-1f88-4a7c-98d1-0646897b1fb4 |
| Version / target | 3.8.5.288 / .NET Framework 4.7, managed CIL AnyCPU |
| Selected types | EncounterData, CombatantData, MasterSwing, Dnum |
| Coverage | Four types, 75 fields, 187 methods; selected metadata/CIL complete |

Combatant DPS (0x06000754) uses its own duration, EncDPS (0x06000755) and EncHPS
(0x06000757) use parent encounter duration, and Encounter DPS (0x060007a1) uses
encounter totals/duration. These getter anchors are not a complete aggregate
specification. The later [ACT host analysis](act-host-compatibility-analysis.md)
restores queue/hook/termination and selected duration rules.

| REA evidence | Scope |
| --- | --- |
| ev_483ce29795ddeff4a55b0da84c7b6db403f2268f646489057ce9360dec1887de | ACT artifact identification |
| ev_555e7c4bfbe752014f8af5c304ea9a52c4b1535cbb6a8a83431122ef8ab3c631 | Four selected types, metadata/CIL and static relationships |
| ev_63e507c4905c68998c6055d806945b84a11676fa6ec1d3b0e94a02e0c8a79d71 | MemberRef 0x0a000234 identifies TimeSpan.get_TotalSeconds |

The original selected-member evidence and local exports are retained under
binaries/analysis; see the later host/SDK reports for parent IDs and fixed hashes.
Original ACT was not run. At this first research stage, the parser DLL was absent
from the local inventory; [SDK analysis](ffxiv-act-sdk-analysis.md) documents its
later addition. Full DPS semantics, actor/owner reconstruction, actual Oodle/game
behavior, regional protocol definitions, and Windows/Linux/FF Logs compatibility
were not established by this source/static analysis.
