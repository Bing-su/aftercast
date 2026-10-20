# M1: headless timer contract

Checked on Linux x86-64 / .NET 10.0.13 on 2026-10-20. This slice advanced the
supplied FFXIV plugin from missing Timer to missing CheckBox. See the
[current status](../README.md) for subsequent compatibility work.
Original initialization, capture and parsing remain
incomplete. JIT resolution can fail before the settings constructor executes.

## Supported behavior

| Contract | Behavior / boundary |
| --- | --- |
| Timer | Component inheritance, default and IContainer constructors; the container owns disposal |
| Interval | Defaults to 100 ms, accepts positive Int32 values, rejects zero/negative values without changing the previous setting |
| Enabled / Start / Stop | Stores the requested enabled state; Start/Stop use the virtual Enabled setter |
| Tick | Event registration/removal and protected virtual OnTick(EventArgs); explicit derived calls run synchronously on the calling thread |
| Disposal | Sets Enabled false before Component disposal, removes the component from its container and preserves Component's disposal events |
| Excluded | Automatic periodic callbacks, timer queues, a UI message pump, native windows, cross-thread dispatch and unlisted Timer members |

Microsoft's [WinForms Timer contract](https://learn.microsoft.com/en-us/dotnet/api/system.windows.forms.timer?view=netframework-4.8)
requires a UI message pump and a single-threaded environment. The adapter has no
pump: Enabled=true is requested configuration, not proof that callbacks run.
It does not substitute a ThreadPool timer, which would change callback threading.
Context7's [/dotnet/docs timer guidance](https://github.com/dotnet/docs/blob/main/docs/standard/threading/timers.md)
also distinguishes these threading models. The official
[Timer source](https://github.com/dotnet/dotnet/blob/b0f34d51fccc69fd334253924abd8d6853fad7aa/src/winforms/src/System.Windows.Forms/System/Windows/Forms/Timer.cs)
was checked for constructors, validation, virtual dispatch and disposal.

Periodic dispatch must be implemented and verified with the host's future caller
thread pump before claiming working timer-driven plugin behavior. A test-derived
OnTick call verifies signatures and event delivery only; it is not a scheduler.
No new Rust API, dependency or original plugin IL change is introduced.

## Evidence and checks

| Evidence / check | Observation |
| --- | --- |
| Pinned FFXIV main plugin | 3.0.3.3; SHA-256 af1031cdba8eeba8f53dd9d618e58d62d4919bfa77db9b31c568bd04cb359093 |
| Static evidence reused | REA ev_176a2d52716f43dc8ba8cd2e95ec966992a516e0a61b8e6d04154381ebe748f8 in ignored binaries/analysis/chart-compatibility-evidence.json; BenchmarkPropertyPage.InitializeComponent 0x060000df |
| Observed member references | Timer(IContainer) 0x0a000196; set_Enabled 0x0a0001a9; set_Interval 0x0a0001aa; add_Tick 0x0a0001ab |
| Independent tests | Defaults, positive interval limits, invalid interval preservation, null container rejection, start/stop, no automatic callback during an enabled observation window, container disposal, synchronous explicit event delivery and removal |
| Genuine reference probe | TimerProbe compiles against net48 WinForms metadata and runs against the adapter, including protected OnTick binding |
| Plugin context fixture | LifecyclePlugin constructs and disposes a container-owned timer through the custom plugin load context |
| Original runtime | Fresh bridge extraction reports missing CheckBox after the timer type barrier; the pinned Linux administrator-query adjustment remains active |
| Packaged consumer | Offline packaging and extracted standalone C# tests pass; with a failing dotnet CLI shim first on PATH, packaged Rust initializes/stops ReadyPlugin and reproduces the original CheckBox failure |
| Regression checks | Managed ABI, Rust workspace/plugin lifecycle tests and strict Linux/Windows-target Clippy pass |

REA metadata/CIL observations are separate from execution. Existing partial
user-string coverage remains a limitation. The packaged original probe output is
retained locally in ignored binaries/analysis/timer-runtime.txt. No original/reference assembly is
distributed. Timer lives in bxivact-winforms's existing prebuilt Forms DLL, which
the Rust host extracts only on Linux. Windows hosts use real WinForms.

Run `mise run test:winforms`, `mise run test:act-binding`, and
`DOTNET_ROOT=/usr/lib/dotnet mise run test:plugins`. Original-reference checks need
the supplied ACT artifact. Original timer callbacks and Windows runtime execution
remain unverified.
