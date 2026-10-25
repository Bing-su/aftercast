# aftercast

An in-process ACT-compatible host for Rust applications. It reuses the existing
FFXIV ACT plugin's parser and ships a prebuilt .NET 10/C# implementation.
Consumers need an installed .NET 10 runtime; no SDK or automatic download is used.

| Start here                                                              | Contents                                                                                                                                  |
| ----------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| [Usage and current support](docs/usage.md)                              | Host/plugin lifecycle, import, original logs, combat snapshots and platform limits                                                        |
| [Architecture](docs/architecture.md)                                    | Ownership, module map, compatibility seams and testing rules                                                                              |
| [Development](docs/development.md)                                      | Rebuild, unified verification and package checks                                                                                          |
| [Domain glossary](GLOSSARY.md)                                          | Encounter, combatant, import, subscription and revision terminology                                                                       |
| [Documentation index](docs/index.md)                                    | Import, log, snapshot and compatibility reference                                                                                         |
| [Independent Linux WinForms model](crates/aftercast-winforms/README.md) | Headless control and chart compatibility                                                                                                  |
| [Standalone TTS and effects](crates/aftercast-tts/README.md)            | Offline SAPI/Speech Dispatcher, optional online Edge, local effects and custom backends; [ACT timer validation](docs/audio-validation.md) |

Linux and Windows fixture-plugin lifecycle, synthetic imports and owned output are tested.
Prior original-plugin checks cover synthetic input and a recorded Zeromus corpus,
with configured action comparisons against genuine ACT. Broader gameplay,
native capture and recording durability remain unverified. Windows original-plugin
import and lifecycle results are recorded in the
[validation report](docs/windows-original-validation.md); the
[ACT compatibility matrix](docs/compatibility-matrix.md) tracks the broader scope.

The project is unpublished and its license remains undecided. Proprietary
reference artifacts are local maintainer inputs and are excluded from Git/packages.
