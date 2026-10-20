# bxivact

An ACT-compatible Rust library that hosts a prebuilt F# component on .NET 10.
Reuses the existing FFXIV ACT plugin rather than replacing its parser.

**Status:** Bridge lifecycle and an initial ACT binary-contract slice work on Linux.
Plugin initialization/parsing and Windows runtime execution remain unverified.

- [Implementation plan](docs/blueprint.md)
- [Development and validation](docs/development.md)
- [M1 contract binding](docs/m1-contract-binding.md)
- [ACT compatibility research](docs/act-host-compatibility-analysis.md)
- [Event delivery research](docs/act-event-delivery-analysis.md)

Consumers need a .NET 10 runtime, not the SDK. Downloads are never automatic.
The project license is undecided; publication is disabled.
