# bxivact

An ACT-compatible Rust library that hosts a prebuilt F# component on .NET 10.
Reuses the existing FFXIV ACT plugin rather than replacing its parser.

**Status:** Host contracts and fixture plugin initialization/shutdown work on Linux.
Original FFXIV initialization now reaches a Windows principal API unsupported on Linux;
parsing and Windows execution remain open.

- [Implementation plan](docs/blueprint.md)
- [Development and validation](docs/development.md)
- [Linux WinForms compatibility crate](crates/bxivact-winforms/README.md)
- [M1 contract binding](docs/m1-contract-binding.md)
- [M1 plugin construction](docs/m1-plugin-construction.md)
- [M1 plugin lifecycle](docs/m1-plugin-lifecycle.md)
- [ACT compatibility research](docs/act-host-compatibility-analysis.md)
- [Event delivery research](docs/act-event-delivery-analysis.md)

Consumers need a .NET 10 runtime, not the SDK. Downloads are never automatic.
The project license is undecided; publication is disabled.
