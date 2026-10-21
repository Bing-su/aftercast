# Windows original-artifact validation — 2026-10-21

The supplied ACT and FFXIV 3.0.3.4 artifacts pass the numeric checks below.
After review corrections, synthetic and pinned-corpus imports also pass their
private-message assertions. Gameplay capture and full plugin compatibility
remain outside these checks.
All files under `binaries` retain their original SHA-256 hashes.

## Inputs

| Artifact | SHA-256 |
| --- | --- |
| `binaries/act/Advanced Combat Tracker.exe`, 3.8.5.288 | `934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657` |
| `binaries/FFXIV_ACT_Plugin_SDK_3.0.3.4/FFXIV_ACT_Plugin.dll` | `f7f0c8894ba1a152b6a453427773496f5948cf412171da773578d8671f643a81` |
| Embedded Parse / SDK Parse, 3.0.3.4 | `efd2db36df247eb23e0a79dd3d19fb2a970f1b52a48901b722fd07728a2aac4a` |
| `D:\repos\cactbot\docs\logs\TheAbyssalFractureExtreme.log` | `a5992e53bef80424ba29d0db4db00d0359c727e00532de0873ce49c67732f240` |

REA static metadata confirms the plugin's .NET Framework 4.8 target, ACT 3.8.5.288
reference and separate DataVisualization reference. The embedded parser's
decompressed hash matches the supplied SDK parser.

## Results

| Check | Result | Observation |
| --- | --- | --- |
| `mise run test:original` | Pass | Genuine ACT constructors, Dnum, configured actions and shared data contracts; model-only control probes run only on Linux |
| Plugin initialization with SDK files alone | Fail | Desktop Runtime lacks `System.Windows.Forms.DataVisualization.dll` |
| Initialization with installed Framework chart DLL | Pass | Additional local chart DLL copied into a disposable plugin directory |
| Synthetic import, three iterations | Pass | Each run: 7 records, 1 encounter, 2 combatants, 400 damage, 0 healing; action, snapshot, subscriber loss and failure-recovery assertions pass |
| Recorded corpus, three iterations | Pass | Each run: 104,744 records, 3 encounters; actor/ally totals, snapshots, restored state and exact subscriber loss agree with the actions |
| Recorded-action replay | Pass | All three transcripts pass against both aftercast and genuine ACT in separate processes |
| Native shutdown before correction | Fail | Missing `Config` directory prevents saving `FFXIV_ACT_Plugin.config.xml` |
| Native lifecycle after correction | Pass | Startup, zero host worker diagnostics, stop and process exit 0; settings file saved |
| Strict private-message assertions after review | Pass | Windows permits an empty startup list and reordered informational notices; callback capture avoids the UI's 100-message limit and still rejects unexpected diagnostics |
| `mise run check` after corrections | Pass | Windows fixture, managed, Rust, formatting and strict Clippy checks |

| Corpus pass | Actions | Combatants | Damage | Healing |
| --- | ---: | ---: | ---: | ---: |
| 1 | 34,685 | 35 | 122,118,617 | 64,058,062 |
| 2 | 34,782 | 36 | 122,118,575 | 64,411,177 |
| 3 | 34,782 | 36 | 122,118,608 | 64,411,140 |

These are observations, not fixed expected totals. The vendor parser retains
private import state and performs stochastic rounding. Every run was checked
against its own committed actions. The initial synthetic/corpus commands failed
at their final diagnostic assertions. Review removed Linux-specific startup
assumptions on Windows and captured the original diagnostic callback directly.
Both commands now pass; unknown errors still fail, and the synthetic input has
no warning allowance.

## Local runtime prerequisite and corrections

The chart image was copied from
`C:\Windows\Microsoft.NET\assembly\GAC_MSIL\System.Windows.Forms.DataVisualization\v4.0_4.0.0.0__31bf3856ad364e35\System.Windows.Forms.DataVisualization.dll`,
SHA-256 `b8b7b8b7a480e9205573f46f440967aeea12a1d4fb543c59b879c8aa80d1e643`.
It is an additional installed runtime input, not part of the SDK or Cargo payload.
Neither redistribution nor visible chart rendering was verified.

The host now creates `AppDataFolder/Config` before plugin construction, with a
lifecycle regression assertion. `.gitattributes` fixes the synthetic log to LF;
Windows CRLF conversion previously broke its pinned hash. Corpus verification
accepts the exact Windows 3.0.3.4 main/parser pair while preserving the original
3.0.3.3 Linux guards. The message checks recognize the exact Deucalion 1.5.1
rename but still reject unknown messages and synthetic simulated-damage warnings.

Logs, three private transcripts, the independent lifecycle probe source and REA
evidence are retained locally under `target/windows-original-3034/`. The plugin
ran from a disposable copy there. Original writer fault-injection checks remain
pinned to SDK 3.0.3.3 and were not applied to 3.0.3.4. Native game capture, private
diagnostic parity, visible UI rendering and Linux regression execution remain unverified.

Review evidence is under `target/windows-local-log-20261016/review-*.txt`,
including the complete [local-log validation](local-log-validation.md).
