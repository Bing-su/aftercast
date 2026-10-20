# M1: headless ListView contract

Checked on Linux x86-64 / .NET 10.0.13 on 2026-10-20. The supplied FFXIV plugin
now passes the missing ListView type barrier and reports missing
System.Windows.Forms.Timer. This does not prove that BenchmarkPropertyPage's
constructor, original initialization or parsing completed: JIT resolution can
fail before the method executes.

## Supported slice

| Contract | Behavior / boundary |
| --- | --- |
| ListView | Control inheritance, Columns, Items, SelectedItems, FullRowSelect, HideSelection, UseCompatibleStateImageBehavior and validated View enum |
| ColumnHeader | Component inheritance, Text, Width metadata and owning ListView |
| ColumnHeaderCollection | Exact nested type, indexed reads, Add/AddRange, ordered enumeration, Remove/Clear and ownership rejection |
| ListViewItem | String-array construction, Name, Text, ListView and nested subitems; null/empty input retains one empty text subitem |
| ListViewItemCollection | Exact nested type, Add returning the item, indexed/keyed reads, ContainsKey, enumeration and Remove/Clear |
| Keys | Live case-insensitive ordinal name lookup, missing/empty key returns null, duplicate names allowed; insertion-order matching |
| Text | Item.Text and SubItems[0].Text share state; other subitems update independently; null text/name normalizes to empty |
| Selection | Empty SelectedItems collection; no native handle or selection mutation API is supported |
| Excluded | Rendering, native handles, events, sorting, virtual mode, complete IList/ICollection contracts, complete WinForms validation/defaults and unlisted APIs |

Microsoft documents [case-insensitive key lookup](https://learn.microsoft.com/en-us/dotnet/api/system.windows.forms.listview.listviewitemcollection.item?view=windowsdesktop-10.0)
and [subitem-zero text](https://learn.microsoft.com/en-us/dotnet/api/system.windows.forms.listviewitem.subitems?view=netframework-4.8.1).
This adapter implements the observed signatures and model operations, not full
Windows control behavior. AddRange adds in order and can leave earlier columns
attached if a later element is invalid. Width is metadata; automatic sizing does
not run. Timer and the remaining settings UI stay outside this slice.

## Evidence and verification

| Evidence / check | Result |
| --- | --- |
| Pinned main plugin | FFXIV ACT 3.0.3.3; SHA-256 af1031cdba8eeba8f53dd9d618e58d62d4919bfa77db9b31c568bd04cb359093 |
| Retained static evidence | REA ev_176a2d52716f43dc8ba8cd2e95ec966992a516e0a61b8e6d04154381ebe748f8 in binaries/analysis/chart-compatibility-evidence.json (ignored); BenchmarkPropertyPage constructor 0x060000d9, timer1_Tick 0x060000da and InitializeComponent 0x060000df |
| Observed references | Column construction/AddRange, keyed row creation/update, subitem access, item enumeration and selected-item reads |
| Independent tests | Defaults, enum rejection, ownership/reuse, nulls, invalid indices, rename-after-add, duplicate keys, subitem text, enumeration and unsupported native handles |
| Genuine reference binding | ListViewProbe compiles against net48 reference metadata and runs against the replacement; no original WinForms DLL is executed or distributed |
| Plugin context | Lifecycle fixture exercises the same row-update pattern through the custom assembly load context |
| Original runtime | Rust initialize example with a fresh bridge directory now reports missing System.Windows.Forms.Timer instead of ListView |
| Packaged consumer | Offline packaging passes; extracted standalone C# tests pass; with a failing dotnet CLI shim first on PATH, packaged Rust initializes/stops ReadyPlugin and reproduces the original Timer failure |
| Regression checks | Managed ABI, Rust workspace/real CLR lifecycle tests and strict Linux/Windows-target Clippy pass; Windows execution is not tested |

The existing pinned Linux administrator-query adjustment remains active. No new
original IL changes or dependencies are introduced. Selected REA metadata/CIL
observations are separate from runtime results; retained user-string coverage is
partial. The packaged original probe output is retained locally in
binaries/analysis/listview-runtime.txt (ignored). The prebuilt Forms DLL remains owned by bxivact-winforms and extracted
only on Linux; Cargo consumers do not invoke the .NET SDK.

Run `mise run test:winforms`, `mise run test:act-binding` and
`DOTNET_ROOT=/usr/lib/dotnet mise run test:plugins`. The original-reference test
requires the supplied ACT artifact. Windows runtime execution, original timer
callbacks, capture and parsing remain unverified.
