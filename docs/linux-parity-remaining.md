# Finite remaining Linux parity inventory

This inventory consolidates the existing ACT targets in [gui-plan](gui-plan.md),
[secondary tools](act-analysis-tools.md) and [source import](source-range-import.md).
It is a completion ledger, not authorization for new unrelated features. A row is
closed only with implemented behavior, synthetic regression evidence and review,
or a precise Windows/product-decision blocker. Audit rows must first identify an
actual missing path; already implemented APIs must be reused rather than rewritten.

| ID  | Existing target                  | Current boundary and completion criterion                                                                                                                                                                                                                                                                     |
| --- | -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| L1  | Plugin-defined table data        | Managed catalog foundation exists; finish bounded values/sort, session-owned API, existing table UI and column preference save/reload. See C1–C4 in [plugin columns](plugin-columns.md).                                                                                                                      |
| L2  | Notification history controls    | Existing list/Show is connected. Finish mark-all-read, clear-read and badge/blinking control according to ACT history behavior; verify counts, stale IDs/sessions and retention boundaries.                                                                                                                   |
| L3  | Source LogEncoding               | Existing reader is strict UTF-8. Implement ACT-grounded explicit supported encoding configuration and decoding/framing/progress/error regressions; do not guess encoding.                                                                                                                                     |
| L4  | Companion logs                   | Implement existing ACT companion registration/dispatch, synchronized ordering, cancellation and multi-reader cleanup with synthetic files. Native plugin-specific effects remain separate QA.                                                                                                                 |
| L5  | History source matching          | Compare ACT folder/character/time matching; complete deterministic source candidate selection plus explicit fallback for missing/ambiguous files. Checkpoint restore and raw reparse remain distinct.                                                                                                         |
| L6  | Remaining raw import options     | Audit the existing plan's file/date boundaries, extend-to-combat-end, clipboard input, character/viewpoint and separate progress surface. Full-file and explicit inclusive-range paths already exist. Close only verified missing options with scoped tests.                                                  |
| L7  | Existing export/report workflows | Audit the planned ACT include-log-lines, HTML/table-XML depth/dimensions and ODBC routes against existing backend APIs; connect actual gaps and report unsupported formats explicitly. No real database, upload or external side effect is authorized by this ledger.                                         |
| L8  | Secondary analysis tools         | Audit the finite tools in [act-analysis-tools](act-analysis-tools.md): death report, search, filtering, timeline, VCR and trigger benchmark. Reuse current encounter/action/source identity; synthetic context and algorithm tests can run on Linux. Native focus/clipboard effects remain QA.                |
| L9  | Existing Options families        | Audit the seven families already listed in gui-plan: general/sound, import/export preferences, main tables, output/display, appearance, selective parsing, data correction. Inventory exact managed/API/GUI gaps before edits; existing settings/trigger/timer implementations are not automatically missing. |

L6–L9 remain coverage audits, not a claim that those families are implemented or
that all are absent. Their finite source lists must be checked before declaring
all Linux-capable ACT work complete; completing L1–L5 alone is insufficient.

| Separate boundary             | Reason it cannot be claimed complete here                                                                                                                                                           |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Windows actual execution      | Native WinForms/CEF/plugin input/focus, capture/reconnection, native timer/audio and desktop interaction require Windows execution. Linux builds/contracts do not substitute for it.                |
| ser7 native clicks            | apps:[] and missing accessibility bus prevent supported click/focus QA. Bounded GTK startup is only a startup check; no system/security changes or bypass are authorized.                           |
| External services             | FF Logs acceptance and real external database/network workflows need separately authorized environments/data; synthetic contract checks do not establish acceptance.                                |
| Partial WinForms bridge       | Existing approved plan only. Implementation requires an explicit request; do not introduce Mono/Wine automatically.                                                                                 |
| Stronger recording durability | Keep approved ACT/vendor behavior and explicit errors. Journaling, acknowledgement/backpressure and crash recovery need the already documented product policy decision; no absolute lossless claim. |
| AI extensions                 | Explicitly deferred and outside ACT parity work.                                                                                                                                                    |
