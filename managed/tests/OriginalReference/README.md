# Original ACT constructor probe

Maintainer-only process with no reference to `ActCompat`. It verifies the original
ACT hash, loads the explicitly supplied headless Forms assemblies, and invokes
genuine aggregate constructors through reflection. It never invokes ACT's entry
point, creates `FormActMain`, or starts game collection. Target files are read only.
It installs ACT's own default string tables through private `ActLocalization.Init()`
and `AddPrebuild()` routines, which only populate dictionaries.

```sh
dotnet run --project managed/tests/OriginalReference/OriginalReference.csproj -c Release -p:RestoreLockedMode=true -- \
  "binaries/act/Advanced Combat Tracker.exe" crates/bxivact-winforms/managed
```

Each stage emits JSON with the actual assembly path or exception. A successful
constructor probe is not an aggregation-equivalence claim.

Append `values` to emit original Dnum formatting, equality, hashing, arithmetic
and derived-type observations. Expected exceptional cases are recorded as data.

Append `actions` to configure explicit fixture damage/healing categories and send
three genuine MasterSwing actions through a genuine ZoneData/EncounterData graph.
The fixture is separate from FFXIV defaults and does not exercise host hooks or
SetEncounter. Aggregate queries report their own failures.

`contracts /absolute/path/Bxivact.LegacyReference.dll` executes the Dnum, combat
state and configured action reference probes against the original ACT implementation after the same
bootstrap. Build the LegacyReference project first; this process still loads no
ActCompat assembly.
