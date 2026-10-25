# Documentation index

| Document                                                               | Purpose                                                                             |
| ---------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| [Usage](usage.md)                                                      | Setup, host/plugin lifecycle and platform support                                   |
| [Architecture](architecture.md)                                        | Source layout, ownership and lifecycle ordering                                     |
| [Dioxus desktop plan](gui-plan.md)                                     | Planned GUI stages, dedicated host thread, command handle and acceptance criteria   |
| [Development](development.md)                                          | Build, verification and package commands                                            |
| [Refactoring measurements](refactoring-performance.md)                 | Query allocations, synthetic workload, changes and verification limits              |
| [Windows validation](windows-validation.md)                            | Windows setup, verified checks and remaining limits                                 |
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
