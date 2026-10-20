# bxivact-winforms

A prebuilt F# `System.Windows.Forms.dll` for Linux .NET hosts, exposed as
`bxivact_winforms::ASSEMBLY`. F# source, the DLL, and contract tests live here.
The crate has no Rust dependencies and never starts .NET or invokes its SDK.

| Current support | Limit |
| --- | --- |
| TabPage / Label signature binding | Both constructors throw PlatformNotSupportedException |
| Unsigned assembly, version 4.0.0.0 | No UI behavior or original WinForms strong-name identity |

The host supplies .NET 10, FSharp.Core 10.0.113, deployment and assembly-context
binding. Windows hosts must use real WinForms. ACT and FFXIV are not dependencies.

From the workspace root, use `mise run winforms:build` to rebuild the checked-in
DLL and `mise run test:winforms` for independent managed contract checks.
The project license is undecided; publication is disabled.
