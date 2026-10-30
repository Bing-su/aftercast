# Selective parsing

ACT 3.8.5.288 `Options_SelectiveParsing.InitializeComponent` defaults to no
selection. Its other modes distinguish limiting graphs/text exports (optionally
also limiting allied totals) from recording actions involving selected actors
(optionally requiring a selected attacker). The public `SelectiveListGetMode`
reports these five cases. Selection names use the original .NET uppercasing
policy. The original `SetEncounter` body rejects a new combat unless at least
one participant is selected when full selective parsing is enabled; an active
selective encounter continues to require a selected participant. The original
`EncounterData.AddCombatAction` object probe verifies victim-only acceptance in
normal selective recording and rejection in strict recording.

The managed host implements the mode/selection at construction and through an explicit owner-thread runtime apply, and
routes newly created encounters through that policy. An existing encounter keeps
its creation policy. Private history records that policy separately, rebuilding
previously accepted actions before restoring it so a different current list cannot
silently discard saved actions. Older private histories without these attributes
retain the previous nonselective interpretation. ACT interchange XML does not
carry the aftercast private attributes.

The app can save mode and one selected name per line as defaults for the next host session, before plugin constructors run. An explicit Apply button sends the current draft to the active session without saving it. Runtime apply is rejected while an import is active; already recorded actions are not rewritten. Selection mutations by plugins work in that session but are not written back to app settings or overwrite the draft.
Managed graph and text-export generation applies the selected list, with allied
totals filtered only in the dedicated mode; admitted reports reject selection
changes during callbacks. A limited-recording encounter cannot recover actions
rejected at capture; reparse from retained original logs where available.
Original body inspection and object probes do not constitute native ACT UI or
live timing verification.

History discovery shows the indexed selective-recording policy separately from
import completion and raw-log retention. The index is an unverified hint until
restore checks it against the archive. Older indexes and archive payloads lacking
the policy remain unknown, including after restoration and re-checkpointing; an
ACT interchange XML import likewise has unknown recording provenance. A known
selective policy means actions may have been excluded, not that exclusion was
observed or that every original source was retained.
