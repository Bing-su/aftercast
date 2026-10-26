# Documentation index

| Document                                                               | Purpose                                                                             |
| ---------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| [Usage](usage.md)                                                      | Setup, host/plugin lifecycle and platform support                                   |
| [Architecture](architecture.md)                                        | Source layout, ownership and lifecycle ordering                                     |
| [Dioxus desktop plan](gui-plan.md)                                     | Library-owned host, GUI stages, ACT analysis/tab workflow targets, evidence limits and acceptance criteria |
| [GUI test plan](gui-plan.md#gui-test-plan)                              | Rust/Host/CLI tests, planned Playwright Desktop automation, regression scenarios and native/platform limits |
| [GUI library selections and research](gui-library-research.md)          | Confirmed GUI/application library choices, validation candidates, configuration/storage policy and source evidence |
| [ACT secondary analysis tools](act-analysis-tools.md)                  | Death/avoidance/special reports, timeline, recast and VCR behavior, data prerequisites and comparison cases |
| [Development](development.md)                                          | Build, verification and package commands                                            |
| [Refactoring measurements](refactoring-performance.md)                 | Query allocations, synthetic workload, changes and verification limits              |
| [Windows validation](windows-validation.md)                            | Windows setup, verified checks and remaining limits                                 |
| [Windows process-isolation validation](windows-process-validation.md) | Current async host, worker cleanup/restart, persisted XML and original FFXIV imports |
| [Windows OverlayPlugin/cactbot follow-up](overlay-cactbot-process-validation.md) | Current CEF/Edge integration, Rust worker WS checks, startup order and legacy subprocess paths |
| [Windows original-artifact validation](windows-original-validation.md) | Supplied ACT/FFXIV checks, extra runtime input and remaining failures               |
| [Local log validation](local-log-validation.md)                        | Optional host-supplied logs, portable invocation and observed private-log results   |
| [Additional plugin validation](additional-plugin-validation.md)        | Official plugin downloads, lifecycle results, constructor fix and concrete blockers |
| [Glossary](../GLOSSARY.md)                                             | Domain terminology                                                                  |
| [File import](import.md)                                               | Input framing, live-state restoration and partial history                           |
| [Encounter archives and history](encounter-archives.md)                | ACT XML/gzip, checkpoints, restart search/restoration and validation limits         |
| [Original log batches](logs.md)                                        | Buffer limits, ownership, loss accounting and binary layout                         |
| [Combat snapshots and GUI queries](snapshots.md)                       | Live counters, encounter list/detail, attack pages, revisions and binary layout     |
| [Compatibility](compatibility.md)                                      | ACT identity, Linux corrections, reference inputs and comparison limits             |
| [ACT compatibility matrix](compatibility-matrix.md)                    | Feature/API implementation, evidence, gaps and next acceptance milestones           |
| [WinForms support](../crates/aftercast-winforms/README.md)             | Headless controls and chart model                                                   |
| [TTS implementation and validation](tts-validation.md)                 | Local SAPI/Speech Dispatcher, optional Edge, ACT routing and tested limits          |
