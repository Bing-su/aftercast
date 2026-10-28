# Lookup by combatant

ACT `FormByCombatantLookup.ShowLookup` copies the selected category’s `All` actions, groups outgoing by victim and incoming by attacker, then adds each action to both the peer’s `All` and named attack. An attack named `All` therefore appears twice in that aggregate. Selecting a skill shows its actions; source category and direction remain explicit. These observations are from the decompiled ACT code, not a native-form run.

Aftercast captures the selected category and matching source logs together under the combat lock using the existing action snapshot lifecycle. Rows and log identities are bounded by the existing 16,384-action and 65,536-log limits; a later action snapshot invalidates the handle and reports an error instead of returning another snapshot’s log. The interface shows a partial-import notice and dropped log count, lets the user select peer, skill and action, and pages action rows. It does not replay logs through the parser.

The first implementation lists peers and skills alphabetically and actions by descending time sorter/time. ACT sorts aggregate tables by configured ACT column comparers; its peer/skill aggregate metric tables and plugin column comparers are not yet reproduced. The original form and physical UI interactions remain unverified on Windows; independent review remains pending because of the existing quota limit.
