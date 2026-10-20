# bxivact-winforms

A prebuilt C# `System.Windows.Forms.dll` for Linux .NET hosts, exposed as
`bxivact_winforms::ASSEMBLY`. The companion chart DLL is exposed as
`DATAVISUALIZATION_ASSEMBLY`. C# sources, DLLs and contract tests live here.
The crate has no Rust dependencies and never starts .NET or invokes its SDK.

| Current support | Limit |
| --- | --- |
| Control hierarchy, TabPage / Label / Form / UserControl construction | Headless objects only; no rendering or native handles |
| Text / Name / IsDisposed and disposal notification | Headless state; no native windows |
| Control tree, geometry, Resize and child disposal | No layout engine or Layout events; batching methods have no work |
| InvokeRequired and synchronous Invoke | Cross-thread calls require a caller-owned synchronization context that dispatches on the original owner thread |
| Chart, Series / ChartArea / Legend metadata and named collections | No data binding, plotting or rendering; BeginInit/EndInit have no invalidation work |
| ListView columns, keyed rows, subitem text and enumeration | No native selection; SelectedItems stays empty; collection interfaces and APIs are partial |
| Timer configuration, container lifetime and event registration | Enabled stores requested state; no automatic Tick callbacks or UI pump |
| Button / ButtonBase, CheckBox.Checked / CheckedChanged, Control.Click wiring | Synchronous programmatic state changes; no native input, automatic checkbox toggling, dialog-button behavior or rendering |
| Unbound ComboBox / ListBox, text/group/link settings controls | Partial collections; no data binding or native interaction; dialogs/clipboard throw explicitly |
| Unsigned assembly, version 4.0.0.0 | No original WinForms strong-name identity |

The host supplies .NET 10, deployment and assembly-context
binding. Windows hosts must use real WinForms. ACT and FFXIV are not dependencies.
Deploy both DLLs in the same managed load context so Chart shares the Control
identity. These are tested signature/model subsets, not full WinForms or charting.

From the workspace root, use `mise run winforms:build` to rebuild the checked-in
DLL and `mise run test:winforms` for independent managed contract checks.
The project license is undecided; publication is disabled.
