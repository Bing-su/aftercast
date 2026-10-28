# ACT encounter timeline contract

The pinned ACT 3.8.5.288 `FormTimeLine.ShowTimeline` uses the selected
encounter's start time and integer-truncated total duration. It reads the `All`
attack from the parser-registered outgoing damage, incoming damage, outgoing
healing and incoming healing categories for each actor. Categories can overlap;
raw swing-type membership alone does not reproduce them. Four group modes are
individual, allied archetypes versus individual enemies, allies versus enemies,
and allies versus individual enemies. The native code obtains `GetAllies(true)`
and uses `GetCombatantType()` for the allied archetypes.

The default sample window is 10 seconds, with a native range of 1–600. Rolling
mode allocates `duration * 2` cells and adds every positive amount at each
integer offset in `[offset, offset + window)` while below duration; negative
integer offsets are skipped. Segmented mode allocates `(duration / window) * 2`
cells and adds to `offset / window + 1`, with C# division truncating toward
zero. In both modes the native chart draws cells `1..=dataPoints`, dividing each
by the full sample window using integer division; consequently its first rolling
cell is not displayed and a final partial segmented bucket may not be displayed.
The addition is ACT Dnum addition, including its behavior after integer overflow.
An out-of-range array access is caught by the original form, rather than being a
valid implicit zero. `TimelineNumeric` makes these errors explicit and places
bounded limits on duration and sample count; limits are aftercast safeguards.

The original also creates a separate marker row for kills, deaths and spells
whose names exist in the timer lookup, and opens an encounter log view near the
selected plot time on right-click. The managed capture now includes those marker rules, all four grouping modes,
category series and retained logs under the same combat lock. The typed worker
API returns an immutable, session-owned snapshot; a new capture retires old
worker pages. The UI supports metric selection (including markers only), grouping,
rolling or segmented samples, per-group visibility, shared or individual scale,
a bounded point viewport and a separate frozen log window for the selected time
range. Request options are snapshotted at admission and stale or canceled UI
responses are ignored.

`timeline-policy` pins the category and arithmetic IL and executes transformed
fixture loops using the original Dnum operators. This does not run the native
form. Synthetic numeric fixtures include equal-time samples, first/last cells,
negative offsets, partial segments, empty input and nonpositive outcomes.
Independent review is pending under the previously reported quota restriction.

Aftercast caps snapshots at 512 actors, 516 groups, 16,384 metric samples per
series, 65,536 marker visits and logs, 2,000,000 aggregate cells, 4,000,000
conservative arithmetic work units, 360,000 seconds and 64 MiB of encoded data.
A bound or native out-of-range condition returns an explicit error and leaves the
previous worker snapshot intact. Empty charts are shown explicitly where ACT's
native drawing can instead fail. Default category and marker names use the
verified English localization; custom localization overrides remain unsupported.

The chart is a simplified static line rendering, not a pixel copy of ACT's native
mirrored filled graph; axis rounding uses a floating-point approximation and
negative overflow values are visually clipped. Exact signed values remain visible
at a selected point. ACT highlights a time in the full log window; aftercast
opens the corresponding frozen half-open time range, retaining source order and
showing the omitted-log count. A cancellation stops waiting for a response but
an admitted worker request may still finish. Windows native execution, physical
GUI interactions and independent review remain unverified.
