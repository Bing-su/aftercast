# M1: headless button contract

Checked on Linux x86-64 / .NET 10.0.13 on 2026-10-20. This slice advances the
supplied FFXIV plugin past missing CheckBox/Button types to missing
System.Windows.Forms.Control.SuspendLayout(). Initialization, capture and parsing
remain incomplete. A JIT resolution failure does not prove that the settings
constructor reached or completed any preceding calls.

## Supported model

| Contract | Behavior / boundary |
| --- | --- |
| ButtonBase | Abstract Control subclass; UseVisualStyleBackColor stores designer metadata, initially true |
| Button / CheckBox | ButtonBase inheritance and existing Control text/name/disposal behavior |
| CheckBox.Checked | Initially false; equal assignments do nothing; changed assignments store the state before invoking OnCheckedChanged |
| CheckedChanged | Registration/removal, synchronous notifications with the checkbox sender and EventArgs.Empty; reentrant changes remain visible; handler exceptions propagate after the state change |
| Control.Click | Registration/removal and protected virtual OnClick(EventArgs), preserving sender/arguments on explicit calls |
| Excluded | Native input, automatic checkbox toggling, CheckState/ThreeState, painting, dialog buttons/IButtonControl, PerformClick, layout and unlisted members |

The style property does not draw a background. CheckBox implements the observed
boolean state operations only. Control.Click is event plumbing, not a simulated
mouse or keyboard action; CheckBox's native/automatic click behavior is not
implemented. No hidden UI thread, rendering or timer scheduling is added.

Microsoft documents [Checked's default and three-state distinction](https://learn.microsoft.com/en-us/dotnet/api/system.windows.forms.checkbox.checked?view=windowsdesktop-10.0),
[CheckedChanged](https://learn.microsoft.com/en-us/dotnet/api/system.windows.forms.checkbox.checkedchanged?view=netframework-4.8.1)
and [UseVisualStyleBackColor](https://learn.microsoft.com/en-us/dotnet/api/system.windows.forms.buttonbase.usevisualstylebackcolor?view=windowsdesktop-10.0).
The official [ButtonBase source](https://github.com/dotnet/dotnet/blob/b0f34d51fccc69fd334253924abd8d6853fad7aa/src/winforms/src/System.Windows.Forms/System/Windows/Forms/Controls/Buttons/ButtonBase.cs)
was checked for its initial style flag.
Context7 was consulted for checkbox state/event guidance; its general UI Automation
results do not establish WinForms signatures, so genuine reference probes verify
those separately.

## Evidence and checks

| Evidence / check | Observation |
| --- | --- |
| Pinned main plugin | FFXIV ACT 3.0.3.3; SHA-256 af1031cdba8eeba8f53dd9d618e58d62d4919bfa77db9b31c568bd04cb359093 |
| Reused static evidence | REA ev_176a2d52716f43dc8ba8cd2e95ec966992a516e0a61b8e6d04154381ebe748f8 in ignored binaries/analysis/chart-compatibility-evidence.json; BenchmarkPropertyPage references CheckBox/Button constructors, Checked access, ButtonBase style metadata and Control.Click wiring |
| Independent checks | Inheritance, metadata, changed/unchanged assignments, event sender/arguments/thread, removal, reentrant changes, handler exceptions, explicit clicks and disposal |
| Genuine references | ButtonProbe compiles against net48 Forms metadata and runs against the adapter, including Button/Control inherited Click and protected OnClick binding |
| Plugin context | LifecyclePlugin constructs both controls, updates Checked and registers/removes handlers through the shared Forms assembly |
| Original runtime | Fresh Rust-host extraction reports missing Control.SuspendLayout instead of CheckBox |
| Packaged consumer | Offline packaging and extracted standalone C# tests pass; with a failing dotnet CLI shim first on PATH, packaged Rust initializes/stops ReadyPlugin and reproduces the original SuspendLayout failure |
| Regression checks | Managed ABI, Rust workspace/plugin lifecycle tests and strict Linux/Windows-target Clippy pass |

The pinned Linux administrator-query adjustment remains active. No additional
original IL or dependencies are changed. REA metadata/CIL observations and runtime
results are separate; retained user-string coverage remains partial. The packaged
original probe output is retained locally in ignored binaries/analysis/buttons-runtime.txt. No original
or reference DLL is distributed. Sources and the prebuilt Forms DLL remain owned
by bxivact-winforms, deployed only on Linux.

Run `mise run test:winforms`, `mise run test:act-binding` and
`DOTNET_ROOT=/usr/lib/dotnet mise run test:plugins`. Original-reference checks need
the supplied ACT artifact. Native button interaction, original initialization
completion, timer callbacks, parsing and Windows runtime execution remain unverified.
