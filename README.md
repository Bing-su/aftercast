# bxivact

An ACT-compatible Rust library that hosts a prebuilt C# component on .NET 10.
Reuses the existing FFXIV ACT plugin rather than replacing its parser.

**Status:** Host contracts and fixture plugin initialization/shutdown work on Linux.
Original FFXIV initializes and imports synthetic and recorded Zeromus fixtures on
Linux. Configured combat-action comparisons pass against original ACT. Broader
gameplay coverage, native capture and Windows execution remain unverified.

- [Implementation plan](docs/blueprint.md)
- [Development and validation](docs/development.md)
- [Linux WinForms compatibility crate](crates/bxivact-winforms/README.md)
- [M1 contract binding](docs/m1-contract-binding.md)
- [M1 plugin construction](docs/m1-plugin-construction.md)
- [M1 plugin lifecycle](docs/m1-plugin-lifecycle.md)
- [M1 chart contract](docs/m1-chart-contract.md)
- [M1 ListView contract](docs/m1-listview-contract.md)
- [M1 timer contract](docs/m1-timer-contract.md)
- [M1 button contract](docs/m1-button-contract.md)
- [M1 controls, startup events and settings](docs/m1-controls-and-settings.md)
- [M1 combat registration and log following](docs/m1-registration-and-log-following.md)
- [M2 combat orchestration and verification](docs/m2-root-combat.md)
- [M2 file import and its boundaries](docs/m2-file-import.md)
- [M3 bounded original-log batches](docs/m3-log-batches.md)
- [M3 owned combat snapshots](docs/m3-combat-snapshots.md)
- [ACT compatibility research](docs/act-host-compatibility-analysis.md)
- [Event delivery research](docs/act-event-delivery-analysis.md)

Consumers need a .NET 10 runtime, not the SDK. Downloads are never automatic.
The project license is undecided; publication is disabled.
