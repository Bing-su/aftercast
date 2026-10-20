# Pinned FFXIV resource newline portability

## Status

The pinned original FFXIV plugin has a confirmed Linux portability gap in its embedded resource reader. The host's import record framing is correct. A resource-reader correction that preserves Windows CRLF splitting is **not implemented**.

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
| Original CIL | `ResourceFile.LoadResourceDictionary` (`06000068`), IL43, calls `System.Environment.get_NewLine` (`0a000056`); IL50 calls `String.Split(string[], StringSplitOptions)`. | Linux splits on LF and retains CR in values. |
| Original parser CIL | Territory strategy `060001dd` resolves territory ID through `ITerritoryList.GetTerritoryNameById`, rather than trusting the display name in the input record. | Correct input framing cannot repair this embedded lookup result. |
| Host source and fixture bytes | `ImportLogReader` handles CR, LF, and CRLF; the synthetic fixture uses LF separators without CR. | Generic input trimming would mask the source of the defect and change unrelated caller data. |

The external override-file branch is different: its separator array already includes `Environment.NewLine`, `"\r"`, and `"\n"` (IL397, IL405, IL413). The confirmed embedded-reader issue does not justify changing override-file parsing.

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

| Future option | Required verification |
| --- | --- |
| Source-level resource-reader replacement | Preserve original resource loading, numeric keys, overrides, and exact CRLF semantics; compare every embedded resource through the original public API. |
| Audited metadata rewrite | Add a genuine CRLF user string while preserving existing metadata tokens and identities; pin original hash, MVID, method bytes, and call site; compare all resources and override behavior. |

Neither option is implemented by this investigation. No new rewriting dependency was introduced, and original target files remain unchanged.

## Evidence and limits

| Evidence | Reference |
| --- | --- |
| ResourceFile managed members and CIL | `ev_c68301cd25a842b0c12407c22a33353209a4ffea6915cb0b6e063d02976c4a71` |
| TerritoryList managed members and CIL | `ev_737c8271e3afef7846961602b97503fc8b0138790e099f25bdfd0f3f3f847eaf` |
| Territory parser strategy | `ev_a76b567c338a8ce357860b6dd6eb9b65777640132cefec25011bbcbd02d8daa1` |
| Local REA ledger | `binaries/analysis/synthetic-import-evidence.json` |
| Full per-resource byte counts and hashes | `binaries/analysis/resource-newline-inventory.txt` |
| Rejected CR-only patch runtime results | `binaries/analysis/resource-cr-patch-probe.txt` |

The analysis artifacts are local, ignored maintainer evidence. The embedded-reader method has 943 IL bytes, SHA-256 `28a2a428fef07111bc6443127640f13c2d74813985c83701d299f3f27ae87ce9`; the rejected patch changed file offset 3991 from `28 56 00 00 0a` to `72 45 04 00 70` in memory only. No corrected all-resource runtime baseline, Windows execution, or alternative plugin-version compatibility is claimed.
