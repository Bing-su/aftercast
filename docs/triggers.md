# Custom triggers

The desktop Custom triggers tab uses the existing Rust/UI-derived Button, Input,
Label and Accordion controls. The same management operations are available from
`HostHandle`; the bridge owns the definitions and callbacks in the worker.

| Operation                       | Contract                                                                                                                                                                                                          |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Create                          | Compile the .NET regex first; reject an existing ACT `category                                                                                                                                                    | regex` key |
| Edit / rename                   | Supply the original key and session-owned revision; reject missing originals, destination collisions and concurrent edits                                                                                         |
| Commit                          | Validate text limits and regex, write/flush a temporary XML file, validate it, replace `state/CustomTriggers.xml`, then publish the runtime change. A failed write preserves the previous runtime definition      |
| Options                         | Original trigger text and import processing are independent from encounter log recording options. Both trigger options persist with definitions                                                                   |
| Reload                          | Explicitly reload the saved file; validate the entire file before changing the registry. Discard the draft before reloading                                                                                       |
| Delete                          | The confirmation captures the displayed key and revision. Deletion removes its results and persists the remaining definitions                                                                                     |
| New draft                       | Refresh can safely advance its revision without losing fields: create never overwrites an existing key                                                                                                            |
| Existing dirty draft            | List selection is blocked. Refresh preserves the draft and its base revision. Review the latest saved values before rebasing, or explicitly keep the draft as a new trigger; the next save still checks for races |
| Navigation / stop               | Pane navigation preserves app-owned work and drafts. Stop clears UI drafts/results; replies from the previous session cannot update a new session                                                                 |
| Commit followed by read failure | A successful save stays acknowledged. A failed subsequent refresh is reported separately; it does not undo the commit or silently resubmit it                                                                     |

An empty category uses ACT's ` General` category. Sound types are None, Beep, WAV
and TTS; missing WAV files fall back to a beep. Timer and result-tab naming share
ACT's TimerName field. Matching uses .NET regex/replacement syntax, with a 100 ms
per-expression timeout. Definition fields are bounded (category/name 512 UTF-16
characters, regex/sound text 4096); NUL is rejected before persistence.

## Matching and results

Pinned ACT 3.8.5.288 source inspection establishes this order:

1. Snapshot parser-processed text before OnLogLineRead; remove TimeStampLen unless
   original text matching is selected. Preserve the source sorter before plugin
   callbacks can change the global counter.
2. Apply active/category-zone selection and the named YOU filter. A defined but
   unmatched YOU group is excluded, as in ACT.
3. Append a tabbed result with time, source text and captures.
4. Return for import records; they never play audio or start live timers.
5. For live records, notify timers, then apply the one-second audio cooldown.

| Result property | Behavior                                                                                                                                                                                                               |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Activation      | Record results must be enabled on the trigger. Import processing defaults off and can be enabled separately                                                                                                            |
| Retention       | At most 4096 records and 4 Mi UTF-16 characters across source/metadata/captures. Individual records over 32768 characters are omitted as a whole; group lengths are checked before materializing large capture strings |
| Omission count  | Reports automatic retention evictions and oversized results. Intentional clear/delete/reload does not count as retention loss                                                                                          |
| Paging          | At most 16 results per owned page, in ascending sequence order. Cursor and definition revision belong to their Host session                                                                                            |
| Lifetime        | Results are transient, not part of encounter checkpoints. Replacement/deletion removes the old trigger's results; reload and explicit clear reset the view without reusing sequence numbers                            |
| Refresh         | Reads current retained data; a page is an owned snapshot, not a live binding or lossless log recorder                                                                                                                  |

## Verification boundaries

`mise run test:gui-state` runs the actual GUI controller methods against a temporary
isolated Host using real opaque revision/cursor values and synthetic records. It
covers dirty-draft selection protection, new/existing conflict recovery, deletion
targets, duplicate admission, late session replies, acknowledged commits and result
page navigation. It does not render or click a native window.

The Windows Playwright suite includes a trigger workflow using the synthetic
TriggerMutationPlugin fixture. `test --list` validates discovery, not execution.
ser7 exposes no native CUA apps and has no connected accessibility bus; actual
GTK/Windows clicks, focus and native audio are not claimed as passed.
