# ACT hosting

This project hosts existing ACT plugins and exposes owned observations to Rust consumers.

## Language

**Host session**:
One active lifetime of the ACT host and its plugin instances. Ending a session does not end the process's CLR lifetime.
_Avoid_: runtime instance

**Plugin**:
A trusted ACT extension that can register hooks, parse logs and configure combat categories. Construction and initialization are distinct lifecycle stages.

**Encounter**:
A segment of combat history containing participants, actions and ACT-derived totals. An encounter may contain multiple active time segments.
_Avoid_: snapshot

**Combatant**:
A named participant in an encounter, with incoming and outgoing combat categories and ally relationships.
_Avoid_: player, since participants can also be enemies

**Log import**:
Delivery of an initially captured file prefix through initialized plugins, with submitted combat actions drained before completion. A failed import can retain partial history.
_Avoid_: replay, when referring only to raw hook delivery

**Original log subscription**:
A bounded observation of nonempty original records selected for ACT hooks. Subscriber loss is distinct from combat-input loss.
_Avoid_: recording, parsed event stream

**Combat snapshot**:
An owned observation of the latest ended encounter and its completion status. Newer versions replace earlier observations.
_Avoid_: encounter archive

**Combat revision**:
A session-scoped version of the latest combat snapshot, including import-status changes. It is not a count of encounters or input records.
