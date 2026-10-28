# Simple encounter graph implementation contract

Reference: ACT 3.8.5.288, SHA256 `934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
Evidence is inspected method IL and independent IL review, not execution of the original WinForms/GDI graph.

## Settings

`Options_Graphing.InitializeComponent` sets OnlyGraphAllies=true (08bc..08cf) and SimpleGraphTotals=true (0828..083b). The constructor sets up these controls; Simple is the initial encounter delegate. `FormActMain.PopulateNewConfig` registers both controls under their Name (0a38..0a7f). `SettingsSerializer.ExportToXml` saves CheckBox Name and Checked.ToString (0257..029e), and ImportFromXml reads Name, parses Value with Boolean.Parse, then assigns Checked (034c..0386). A valid saved false overrides the true default; a missing entry leaves the constructed value. Invalid settings must not be guessed as false; serializer/load error handling is distinct from graph arithmetic.

## Ordered selection and values

`GenEncounterGraph` copies original encounter actors, sorts using the selected CombatantData column comparer, then reverses (0027..0082). Do not supply an invented stable tie-break: ties inherit native List.Sort behavior. Sort failure logs Failed Sorting and returns a message bitmap (0051..0077).

If OnlyGraphAllies is checked, get encounter.GetAllies unless encounter.GetIsSelective && GetIgnoreEnemies, in which case use the full sorted actor list (0088..00b5). Filter by membership only when that list is nonempty (01d1..01f5). An empty allies result therefore includes every actor. Independently, selective-export mode applies SelectiveListGetSelected(actor.Name) (01f7..0216). Its lookup and add paths use current-culture ToUpper, not an ordinal registry-key comparison. The selected-list boolean must be true. Retain original objects for comparer, GetAllies membership, and SQL callbacks.

For each admitted actor call the selected column GetSqlData, then Double.Parse(text, usCulture) (0219..0237). Only value > 0 is appended with its original name and added to the double sum (023e..0265). The unordered comparison excludes NaN, zero and negatives; positive infinity passes. Exceptions in this row block are skipped (0269..026a). Scope/owner validation failures introduced by aftercast must remain fatal, even when the plugin callback itself is a row failure.

No accepted values returns a message before axis/totals: distinguish no original actors from actors with no accepted damage (0300..0355). Empty selected rows are not a numerical graph with a fabricated axis.

## Axis, totals and representation

Axis maximum starts at double 1. For each accepted value greater than the current maximum, assign Ceiling(value); finally multiply by 1.1 (0379..03d6). Fractional values remain double; do not apply the signed action-bar integer ceiling or average windows. Bar geometry scales value by plotHeight/axisMaximum.

When totals are enabled, use the accepted double sum and sum / acceptedCount. The exact template is `Total: {0:0,0}\nAVG: {1:0,0.00}` with an actual LF; String.Format uses current culture, while SQL parsing uses usCulture (0949..097a). Suffix labels separately truncate via conv.i8 and call CreateDamageString with suffix/decimal controls (070f..0745); tooltip values use `{0}: {1:#,0}` (06cc..06e4). Neither label path changes the underlying bar value.

Positive infinity, overflowed sums, axis multiplication overflow, and values outside Int64 label/grid conversion are unsafe geometry cases. Native IL allows them into later conv.i8/Convert.ToInt64/GDI operations and has an outer message-bitmap failure path (0a12..0a35). A bounded static implementation should reject nonfinite geometry explicitly and document that diagnostic difference. Never claim native success for these cases. Runtime parsing is also platform-sensitive: the Linux .NET 10 en-US infinity symbol observed by the fixture differs from the string Infinity, and numeric overflow parsing can differ from .NET Framework. These require Windows original-runtime probes before claiming textual parse equivalence.

## Executable fixtures

`managed/tests/fixtures/graphs/simple.json` pins explicit ascending comparer output with no ties, reverse order, allies/selected flags, finite axes, total/average, and current-culture totals text. `SimpleGraphContractChecks` executes a test-only transcription using actual .NET Parse, Ceiling, arithmetic and Format. It is an oracle for the stated transform, not production code or an independent original runtime execution. Run after `mise run managed:test-runner`:

```sh
DOTNET_ROOT=/usr/lib/dotnet dotnet managed/tests/ManagedTests/bin/Release/net10.0/Aftercast.ManagedTests.dll simple-graph-contract
```

The normal numeric model is now sufficiently specified for a bounded managed implementation with explicit only-allies/selective/totals options. Remaining native-only questions concern GDI layout, fonts/colors, custom graph delegates, suffix rendering and original-runtime extremes. No user product decision blocks the finite numeric core. Report/API/GUI wiring and pure production calculation remain follow-up work; this contract does not mark Simple supported.
