# bxivact-winforms

A prebuilt F# `System.Windows.Forms.dll` for Linux .NET hosts, exposed as
`bxivact_winforms::ASSEMBLY`. F# source, the DLL, and contract tests live here.
The crate has no Rust dependencies and never starts .NET or invokes its SDK.

| Current support | Limit |
| --- | --- |
| Control hierarchy, TabPage / Label / Form / UserControl construction | Headless objects only; no rendering or native handles |
| Text / Name / IsDisposed and disposal notification | No message loop, layout, control tree or cross-thread dispatch |
| Unsigned assembly, version 4.0.0.0 | No original WinForms strong-name identity |

The host supplies .NET 10, FSharp.Core 10.0.113, deployment and assembly-context
binding. Windows hosts must use real WinForms. ACT and FFXIV are not dependencies.

From the workspace root, use `mise run winforms:build` to rebuild the checked-in
DLL and `mise run test:winforms` for independent managed contract checks.
The project license is undecided; publication is disabled.
