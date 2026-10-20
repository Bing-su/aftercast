# M1: controls, startup events and settings

Checked on Linux x86-64 / .NET 10.0.13 on 2026-10-20. This continuation advances
the original FFXIV plugin past SuspendLayout, settings controls, startup event
delegates and SettingsSerializer, to missing ACT DamageTypeDef. Neither completed
original initialization nor parsing is claimed. JIT resolution failures do not
establish completion of preceding calls.

## Implemented contracts

| Area | Behavior |
| --- | --- |
| Geometry | Shared Control/Chart Size, Width/Height, Location, Resize, margin and designer scale/dock/anchor metadata |
| Control ownership | Canonical Controls collection, live name lookup, reparenting, cycle rejection, parent visibility/enabled inheritance and child disposal |
| Failure cleanup | Every captured child is attempted even after a disposal exception; aggregate errors remain observable; disposed old HostEnvironment cannot clear a later host's globals |
| Layout | SuspendLayout, ResumeLayout and PerformLayout do no work because this model has no layout engine or Layout events |
| Threading | InvokeRequired compares creation/caller thread; Invoke executes only on that thread; cross-thread dispatch fails explicitly without a UI pump |
| Selection | Unbound ComboBox/ListBox items, selection indices, ComboBox item lookup, selected text, change events and clearing; no data binding or full collection interfaces |
| Other settings controls | GroupBox, TextBoxBase/TextBox, LinkLabel event signatures; dialogs, message boxes and clipboard throw instead of fabricating UI results |
| ACT startup | Original log payload fields/constructors, BeforeLogLineRead, LogFileChanged, UpdateCheckClicked and writable nested date-parser delegate field |
| Host policy | GetAutomaticUpdatesAllowed returns false; application updates belong to the consumer. Default timestamp parsing throws until a plugin installs its parser |
| Parser configuration | TimeStampLen, LogPathHasCharName and ZoneChangeRegex state; not a complete log ingestion engine |

ControlCollection construction is internal: only the canonical Controls property
may own children. Review caught that detached public collections could otherwise
lose parent/cleanup ownership. Native handles, rendering, fonts, automatic timer
ticks, layout calculations and cross-thread dispatch remain unsupported. Complete
WinForms validation, defaults and interfaces are not claimed.

Microsoft documents [control reparenting](https://learn.microsoft.com/en-us/dotnet/api/system.windows.forms.control.controlcollection.add?view=windowsdesktop-10.0),
[layout batching](https://learn.microsoft.com/en-us/dotnet/api/system.windows.forms.control.suspendlayout?view=netframework-4.8.1)
and [unbound combo selection](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/controls/combobox-control-overview-windows-forms).
Context7 was consulted for UI thread affinity; the adapter explicitly rejects
dispatch it cannot provide.

## Settings XML

| Operation | Supported behavior |
| --- | --- |
| Registration | Exact CheckBox/ComboBox/TextBox types; first duplicate registration wins; other types fail visibly |
| ExportToXml(XmlTextWriter) | Writes Name/Value child records; caller owns wrapper, writer and file |
| ImportFromXml(XmlTextReader) | Consumes the positioned wrapper; returns per-record error count; malformed XML propagates; unknown tags are ignored; known invalid records log errors and continue |
| Combo finalization | Temporarily permits editable text, restores the previous style, finds exact item text; last matching item wins |
| XML boundary | External resolution disabled and DTD processing prohibited before consuming the wrapper; prior caller-side reads cannot be undone |
| Intentional correction | Style restoration retains control references, so registration keys may differ from Control.Name; original ACT's name/key mismatch can leave a combo editable |
| Limits | No ACT parent-field serialization, fonts, colors or other controls. Finalization failures are logged rather than swallowed |

The genuine-reference probe covers escaping, roundtrip, duplicate registration,
different key/control names, invalid records, continued import, wrapper consumption
and unsupported controls. Original InitPlugin catches and logs its own failures;
the bridge still treats same-thread ACT error logs as initialization failure.

## Evidence and execution

| Source | Scope |
| --- | --- |
| Main plugin | FFXIV ACT 3.0.3.3, SHA-256 af1031cdba8eeba8f53dd9d618e58d62d4919bfa77db9b31c568bd04cb359093 |
| ACT reference | 3.8.5.288, SHA-256 934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657 |
| UI/startup evidence | REA ev_38e497b406b100a7867a95426a1d4d164a5fa2fb76ee687eff09fc2f5533a48f and ev_85a1b4c4b05c13db0f4616382503bc2219728b9ed8a3dc02a15ef545d5e4b6ae |
| ACT payload/delegates | REA ev_9318bb958a04bdb057f7e54dbb013dc7f8dca37d81ebd722adf83d6532e4899b and ev_a43502c2c8117d4ddc9185e4fca2062b9d4383f19773f3e9bc387e53e585d98d |
| Serializer | REA ev_3faab33b6a6d2590e5f78d99148584527c7d4f77c1e036fafdc7eb7014867e3c |
| Retained ledger | Ignored binaries/analysis/continuous-startup-evidence.json; previous chart evidence also retained. User-string coverage remains partial |

Probes now execute a fresh copy of the plugin directory: original startup can
move/delete Deucalion distribution files. Supplied binaries remain untouched.
The existing pinned in-memory Linux administrator-query adjustment is unchanged.
The ACT clear-button lookup is optional and returns no match on the headless
root; no fake ACT UI tree is created.

Run `mise run test:winforms`, `mise run test:act-binding` and
`DOTNET_ROOT=/usr/lib/dotnet mise run test:plugins`. Original artifact/reference
checks require the local supplied files. Native process access, worker health,
combat aggregation, log import/export and Windows execution remain separate work.
