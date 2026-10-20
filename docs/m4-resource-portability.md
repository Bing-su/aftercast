# Pinned FFXIV resource newline portability

## Status

The pinned original FFXIV plugin has a confirmed Linux portability gap in its embedded resource reader. A guarded Linux-only in-memory correction now restores its Windows CRLF splitting semantics. Original resource text and deployment files remain unchanged; the host does not trim caller values.

| Scope | Identity |
| --- | --- |
| Artifact | `FFXIV_ACT_Plugin.Resource.dll` |
| SHA-256 | `10dc07de16da355b96154cb11550cae7146a0124730e87c71466e84fd63fb33c` |
| MVID | `3df23e26-6e4d-451f-a603-4ef197526e7b` |
| Investigation date | 2026-10-21 |
| Execution boundary | Copied target assemblies; isolated reflection probes; no game, plugin entry point, or original deployment writes |

## Observed behavior

| Evidence type | Observation | Implication |
| --- | --- | --- |
| Direct original public API | `new ResourceFile("Generated.TerritoryList_English.txt", false).Dictionary[129u]` returns `"Limsa Lominsa Lower Decks\r"` on Linux. | The suffix exists before host zone callbacks or snapshot encoding. |
| Embedded bytes | Territory resource row is `129\|Limsa Lominsa Lower Decks\r\n`. | The resource uses Windows record separators. |
| Original CIL | `ResourceFile.LoadResourceDictionary` (`06000068`), IL43, calls `System.Environment.get_NewLine` (`0a000056`); IL50 calls `String.Split(string[], StringSplitOptions.RemoveEmptyEntries)`. | Linux splits on LF and retains CR in values. |
| Original parser CIL | Territory strategy `060001dd` resolves territory ID through `ITerritoryList.GetTerritoryNameById`, rather than trusting the display name in the input record. | Correct input framing cannot repair this embedded lookup result. |
| Host source and fixture bytes | `ImportLogReader` handles CR, LF, and CRLF; the synthetic fixture uses LF separators without CR. | Generic input trimming would mask the source of the defect and change unrelated caller data. |

The embedded override-resource branch is different: its separator array already includes `Environment.NewLine`, `"\r"`, and `"\n"` (IL397, IL405, IL413), with `RemoveEmptyEntries` at IL419. It enumerates manifest resources named `Generated.Override.global_*`, not external files. This branch and its invariant-case join remain unchanged.

## Complete embedded text inventory

All 53 embedded `.txt` resources were decoded with strict UTF-8 and inspected. The counts below describe resource bytes, rather than claiming full dictionary equivalence.

| Newline class | Resource count | Detail |
| --- | ---: | --- |
| CRLF only | 50 | At least one CRLF; no lone LF or bare CR |
| Mixed CRLF and LF | 1 | `Generated.ActionList_German.txt`: 45,554 CRLF, two lone LF, zero bare CR |
| No newline | 2 | `Generated.ItemList_English.txt` and `Generated.MountList_English.txt`, five bytes each |
| Total | **53** | No resource contains a bare CR |

Under an LF split, 50 resources have two-field rows whose values retain CR: 414,307 rows in total. The German action resource produces 45,556 nonempty LF-separated rows versus 45,554 CRLF-separated rows. Therefore replacing CRLF semantics with a split on either character would also change the interpretation of embedded LF content.

| Resource | SHA-256 |
| --- | --- |
| TerritoryList English | `23d33813c8a3d92b2df3fa981800650f9d293bfd2dd518bbe06cbeeba4449f7a` |
| ActionList German | `36c8fa4db9594a9428414c5547d28365b60f9383778119796da689fe468712fa` |
| Each newline-free resource | `ad941d86cd1cef573f8775b8ebded6eb437739e26f83f9e3e782a50bf7dd40fa` |

## Rejected narrow patch and public mutation workaround

The original `#US` heap contains 66 entries, including `"\r"` (`70000445`) and `"\n"` (`70000449`), but no `"\r\n"` literal. A five-byte `call` to `ldstr` replacement cannot select a pre-existing CRLF token.

A controlled in-memory copy replaced only IL43 with `ldstr 70000445`. Actual public `ResourceFile.Dictionary` calls then failed for **51 of 53 resources**; the two newline-free resources returned empty dictionaries. CR splitting leaves leading LF on subsequent numeric keys, and `Convert.ToUInt32("\n129", 10)` throws `FormatException`. The reader's subsequent failure path surfaces `ArgumentNullException` for a null LINQ source. This candidate is rejected.

Mutating a dictionary returned by the public getter is also insufficient. Two reads of the same `ResourceFile.Dictionary` return distinct dictionaries. Replacing entry 129 in one returned dictionary does not affect the next read. `ResourceFile` has only private instance `_fileName` and `_idIsHex` fields; `TerritoryList` retains its own private instance `_territories` dictionary. The inspected assembly exposes no public global resource dictionary registry that would make a single public mutation a stable correction.

## Implemented bounded correction

`LinuxResourceCompatibility.cs` replaces only IL43–85. It constructs `"\r\n"`
from the existing CR/LF user strings through the existing
`String.Concat(string, string)` member reference. To fit that sequence without
moving code, it constructs the same first selector delegate directly using the
original singleton and method, instead of consulting/populating its private
cache. This adds one selector allocation per dictionary read; it does not change
the selector, its target or the public dictionary contract. Five NOPs retain
the original IL86 boundary.

| Guard or invariant | Pinned value |
| --- | --- |
| Original image size | 8,671,232 bytes |
| Method RVA / file IL offset | 11,616 / 3,948 |
| Method / maximum stack | 943 IL bytes / 6 |
| Locals signature | `1100000e`, initialized locals |
| Mutation window | 43 bytes at IL43–85; every other image byte unchanged |
| Metadata, fat header and EH | Same tokens, size, offsets and five exception regions |
| Control flow | Replace the first private cache branch with direct delegate construction; retain IL86 and all later instructions/targets |
| Patched method SHA-256 | `f4f48266e0b23c7d61c12cd3cf1b10c02d2dc904f33b4e1230b54cfe8bf2d2ba` |
| Patched image SHA-256 | `3767471753a70412b92af4e0978fec74f92eb04e9c1aad4f97207055f9a9b780` |
| Deployment | Known main-plugin hash on Linux only; bounded Costura decompression and `LoadFromStream` in the plugin context |
| Unknown or previously rewritten image | Rejected before metadata inspection |

The production adapter pins the complete original image hash, MVID, declaring
type/method/signature, header, locals, EH, complete IL and the reused literal,
concat and selector identities. It introduces no rewriting dependency or general
patching API. Windows continues to load the original resource reader.

## Maintained verification

`ResourceCompatibilityChecks` calls the actual patched public `ResourceFile.Dictionary`
getter for every embedded text resource with both radix flags. Expected values
come from an independent CIL-derived reference that uses exact Windows separators;
this is not a genuine Windows execution.

| Check | Result |
| --- | --- |
| Complete resource/radix matrix | 53 resources × 2 flags = 106 cases |
| Exact successful dictionaries | 44; every key/value checked ordinally |
| Existing invalid-numeric API paths | 62; preserve the caught-parse/null-LINQ-source failure for wrong radix or override-as-dictionary inputs |
| Embedded override resources | 23, with 645 applied replacements; retained invariant-case join |
| German action resource | 45,554 CRLF rows; embedded lone LF content preserved |
| Territory 129 | Exactly `Limsa Lominsa Lower Decks`, without CR |
| Layout and ownership | Whole-image comparison outside the sole mutation window; unchanged original array/file; empty/unknown/re-patched rejection |
| Actual original-plugin binding | In-memory Resource assembly plus exact patched method hash; synthetic imported snapshot requires the clean zone name |

## Evidence and limits

| Evidence | Reference |
| --- | --- |
| ResourceFile managed members and CIL | `ev_c68301cd25a842b0c12407c22a33353209a4ffea6915cb0b6e063d02976c4a71` |
| TerritoryList managed members and CIL | `ev_737c8271e3afef7846961602b97503fc8b0138790e099f25bdfd0f3f3f847eaf` |
| Territory parser strategy | `ev_a76b567c338a8ce357860b6dd6eb9b65777640132cefec25011bbcbd02d8daa1` |
| Local REA ledger | `binaries/analysis/synthetic-import-evidence.json` |
| Full per-resource byte counts and hashes | `binaries/analysis/resource-newline-inventory.txt` |
| Rejected CR-only patch runtime results | `binaries/analysis/resource-cr-patch-probe.txt` |
| Corrected prototype results | `binaries/analysis/resource-crlf-prototype-results.jsonl` |
| Independent selector/override CIL | `ev_1e9386e27519fc5442c51f50fd39c96692e6ad7465cde323751202618e3bf32e`, `ev_cb950eb31d57925caa38db8638e58980fa9e72f0e27de71fed6d64a0c0777548` in `binaries/analysis/resource-crlf-evidence.json` |

The analysis artifacts are local, ignored maintainer evidence. The original
embedded-reader method has SHA-256
`28a2a428fef07111bc6443127640f13c2d74813985c83701d299f3f27ae87ce9`.
The rejected CR-only experiment is distinct from the implemented 43-byte correction.
Windows execution, external override-file handling and alternative plugin-version
compatibility remain unverified.
