# M1: headless chart contract

Checked on Linux x86-64 / .NET 10.0.13 on 2026-10-20. The FFXIV main plugin now
passes the missing DataVisualization assembly/member barriers. The subsequent
[ListView](m1-listview-contract.md) and [timer](m1-timer-contract.md) slices
advance the failure to CheckBox.
Initialization, capture and parsing are incomplete.

## Implemented model

| Contract | Supported behavior / boundary |
| --- | --- |
| Assembly | Unsigned System.Windows.Forms.DataVisualization, version 4.0.0.0; shares the adapter's Control identity |
| Chart | Control-derived lifetime, Series/ChartAreas/Legends, DataSource, BorderlineDashStyle and Size metadata |
| Series | Named element and DataPointCustomProperties inheritance; XValueMember, YValueMembers, XValueType, ChartArea, Legend, YValuesPerPoint, IsVisibleInLegend |
| Axes and legends | AxisX/AxisY, label formatting, IsLabelAutoFit, Maximum and legend Enabled state |
| Named collections | Ordered Collection<T> inheritance, string lookup, generated names, duplicate-name rejection, rename validation and single-collection ownership |
| Initialization batches | BeginInit/EndInit perform no work because this model has no native invalidation/rendering pump |
| Unsupported behavior | DataBind throws PlatformNotSupportedException; native handles/rendering remain unsupported; full element interfaces, serialization and chart APIs are outside this slice |

This is metadata storage, not a renderer or chart-data engine. Properties not listed
here and complete WinForms/chart validation/default rules are not claimed.
Microsoft's [Chart contract](https://learn.microsoft.com/en-us/dotnet/api/system.windows.forms.datavisualization.charting.chart?view=netframework-4.8)
confirms the Control base and ISupportInitialize interface. Its
[implementation](https://github.com/dotnet/winforms-datavisualization/blob/main/src/System.Windows.Forms.DataVisualization/ChartWinControl.cs)
was checked for initialization invalidation and the NotSet border default.
Context7 also confirms that real WinForms uses the Windows Desktop SDK; adding
that SDK is not a Linux implementation strategy.

## Evidence and progression

| Evidence | Observation |
| --- | --- |
| Main FFXIV plugin | 3.0.3.3; SHA-256 af1031cdba8eeba8f53dd9d618e58d62d4919bfa77db9b31c568bd04cb359093 |
| BenchmarkPropertyPage selection | REA ev_176a2d52716f43dc8ba8cd2e95ec966992a516e0a61b8e6d04154381ebe748f8; constructor 0x060000d9 and InitializeComponent 0x060000df reference chart/model members and ListView |
| net48 chart reference | Microsoft.NETFramework.ReferenceAssemblies.net48 1.0.3; SHA-256 42cd5972f09e5d3de4c6a9046712475dabe8243bfd9fdd2c6f19f6bc88555772; MVID 419d8d35-b119-4757-b823-7ea7fbb72794 |
| Reference selection | REA ev_5ed311918063087f0aaca66af9b775ece6f0c1dee05e0a646d6ec79f1c5b7568; generic collection and Series inheritance/signatures |
| Runtime: Chart type only | Assembly resolves, then InitPlugin reports missing Chart.get_Series |
| Runtime: chart/model signatures | Chart.get_Series resolves, then InitPlugin reports missing System.Windows.Forms.ListView |
| Retained evidence | binaries/analysis/chart-compatibility-evidence.json, ignored locally; static observations do not prove completed constructors |

The same pinned Linux administrator-query adjustment remains active. No additional
original IL is changed by this chart slice, and no original binaries/reference
assemblies are distributed. Tokens belong only to their recorded artifact hashes.
Selected metadata/CIL is execution-free; runtime probes are separate observations.
REA preserves partial user-string coverage for malformed terminal flags; no claim
of complete literal recovery is made.

## Ownership, deployment and checks

| Area | Implementation / check |
| --- | --- |
| Crate ownership | C# sources and both prebuilt DLLs remain in bxivact-winforms; no ACT/Rust hosting dependency is added |
| Rust deployment | DATAVISUALIZATION_ASSEMBLY exposes chart bytes; bxivact extracts both adapters on Linux only |
| Managed contexts | Platform binds both DLLs in the bridge context; PluginContext shares the chart assembly rather than resolving another private copy |
| Independent tests | Control identity, model persistence, rename/duplicate/ownership behavior, disposal and visible DataBind failure |
| Original-reference probe | ChartProbe compiles against genuine net48 WinForms/chart metadata and executes against the two replacement DLLs |
| Rust fixture | A lifecycle plugin uses the genuine chart signatures through PluginContext and casts Chart to the shared Control |
| Isolated source build | The extracted crate rebuilds and runs its control/chart tests; both project references are explicit after an isolated build exposed a missing transitive reference |
| SDK-free package | With a failing dotnet CLI shim first on PATH, the extracted Rust host initializes and stops ReadyPlugin and reproduces the original plugin's ListView boundary |

Use `mise run test:winforms`, `mise run test:act-binding`, and
`DOTNET_ROOT=/usr/lib/dotnet mise run test:plugins`. The original-reference check
requires the supplied ACT artifact. Use a fresh bridge extraction to rerun the
trusted original plugin. Windows runtime execution and a functioning original
settings page are unverified. The later ListView slice handles the observed list model;
the later timer contract advances the runtime boundary to CheckBox.
