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

The managed host currently implements the mode/selection at construction and
routes newly created encounters through that policy. An existing encounter keeps
its creation policy. Private history records that policy separately, rebuilding
previously accepted actions before restoring it so a different current list cannot
silently discard saved actions. Older private histories without these attributes
retain the previous nonselective interpretation. ACT interchange XML does not
carry the aftercast private attributes.

The app persists mode and one selected name per line; the settings take effect
on the next host session, before plugin constructors run. Selection mutations
by plugins work in that session but are not written back to the app settings.
Managed graph and text-export generation applies the selected list, with allied
totals filtered only in the dedicated mode; admitted reports reject selection
changes during callbacks. A limited-recording encounter cannot recover actions
rejected at capture; reparse from retained original logs where available.
Original body inspection and object probes do not constitute native ACT UI or
live timing verification.
