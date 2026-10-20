# bxivact

An ACT-compatible Rust library that hosts a prebuilt F# component on .NET 10.
Reuses the existing FFXIV ACT plugin rather than replacing its parser.

**Status:** M0 bridge lifecycle works on Linux. Plugin loading, parsing, and ACT
compatibility are not implemented yet. Windows runtime execution is unverified.

- [Implementation plan](docs/blueprint.md)
- [Development and validation](docs/development.md)
- [ACT compatibility research](docs/act-host-compatibility-analysis.md)
- [Event delivery research](docs/act-event-delivery-analysis.md)

Consumers need a .NET 10 runtime, not the SDK. Downloads are never automatic.
The project license is undecided; publication is disabled.
