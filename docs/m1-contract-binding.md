# M1: first ACT binary-contract binding slice

Implemented and checked on Linux/.NET 10 on 2026-10-20. This is the first M1
prerequisite, not completion of plugin initialization or ACT aggregation.

## What is established

| Question | Evidence |
| --- | --- |
| Can an original ACT reference bind to our F# assembly? | A probe compiled against the original EXE runs against the replacement DLL and calls selected Dnum members |
| Does reference identity match? | Name Advanced Combat Tracker, version 3.8.5.288, neutral culture, public key token a946b61e93d97868 |
| Is the original ACT executed? | The original file is read as PE metadata/compiler reference only; the runtime probe checks that Dnum comes from crates/bxivact/managed/Advanced Combat Tracker.dll |
| Does this initialize FFXIV? | No. IActPluginV1, WinForms, host globals, plugin dependency resolution and InitPlugin remain pending |
| Can Rust receive a useful managed failure? | Command/status plus bounded UTF-8 diagnostic text; an unknown-command regression also checks the session still works afterward |

## Identity and public signing

ActCompat now outputs Advanced Combat Tracker.dll instead of Bxivact.ActCompat.dll.
The 160-byte public key is extracted from the hash-pinned reference and checked in
as ACT.publickey. F# uses public signing, not the unavailable original private key.
This preserves reference identity for the tested modern runtime, not an authentic
original signature or proof of trust. It does not establish .NET Framework support.

Context7's .NET documentation describes [public signing](https://github.com/dotnet/docs/blob/main/docs/csharp/language-reference/compiler-options/security.md)
and [modern runtime strong-name validation](https://github.com/dotnet/docs/blob/main/docs/standard/assembly/create-public-private-key-pair.md).
The F# compiler's own --help exposes --publicsign; the actual binding result is
verified rather than inferred from the C# documentation.

## Supported Dnum slice

These are restored from the original 3.8.5.288 CIL. Values represent ACT's numeric
contract, not parsed FFXIV packets.

| Member | Implemented behavior | Original token |
| --- | --- | --- |
| .ctor(int64) | Preserve number; empty explicit damage string | 0x06000699 |
| .ctor(int64,string) | Preserve number and supplied text | 0x0600069a |
| op_Implicit(Dnum) -> int64 | Return Number | 0x0600069b |
| op_Implicit(int64) -> Dnum | Preserve values >= -10; clamp lower values to -9 | 0x0600069c |
| NoDamage/Miss/Unknown/Death/ThreatPosition | Convert 0/-1/-9/-10/-11 through the implicit operator; ThreatPosition therefore has Number=-9 | 0x060006a0-a4 |
| Number | Read-only int64 property | 0x060006a5 |
| DamageString | Mutable explicit text; null/empty delegates to ToString, currently unsupported | 0x060006a6-a7 |
| DamageString2 | Mutable string, initially null | 0x060006a8-a9 |

Direct construction of -11 or Int64.MinValue is not normalized. Implicit conversion
of those numbers is normalized. The original-reference probe checks both paths
and Int64.MaxValue. Numeric getters/conversions are not limited to 32-bit values.

Formatting/localization, addition, comparison/IComparable, equality, and hashing
remain outside this slice. ToString(), Equals(), and GetHashCode() explicitly
throw NotSupportedException so inherited Object behavior cannot masquerade as
ACT semantics. The other missing member signatures are not yet provided. Do not
use this as a complete Dnum implementation or a working combat engine.

## Reproduce the experiment

| Step | Action |
| --- | --- |
| Prepare reference | Supply binaries/act/Advanced Combat Tracker.exe matching SHA-256 934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657 |
| Run | mise run test:act-binding |
| Identity check | PrepareActReference.fsx reads metadata, validates hash/name/version, checks/extracts public key |
| Original-reference compile | LegacyReference.fsproj references original metadata with Private=false; no replacement project reference |
| Replacement execution | ActBindingTests.fsx explicitly loads our DLL, checks reference/full identity/location and read-only Number, then runs the compiled probe |

Checks throw on failure in Release; they do not depend on DEBUG assertions.
The original proprietary binary and REA exports remain ignored and are not in the
Cargo package. Consumers neither build this fixture nor need the original ACT
file. They use the checked-in, prebuilt compatibility component.

## Diagnostics and review

| Review finding | Change / validation |
| --- | --- |
| Numeric status alone hid managed failures | Added ReadError and Error::Managed.message; Rust keeps status/command and validates returned length/UTF-8 |
| A small diagnostic buffer could encourage partial writes | ReadError reports required bytes and writes nothing when short; F# tests verify the caller's header remains unchanged |
| Old diagnostic text could leak into a successful call | Invoke clears prior error; test verifies zero error length after successful start |
| Release probe checks must remain active | Explicit failing checks replace assertion-only validation in the compiled probe and managed script |
| Exception handlers must cover native entry boundaries | Both delegates catch ordinary managed exceptions including lock acquisition failures; diagnostic formatting has its own fallback |

Diagnostics are read immediately on the creation thread; the bridge is not a
concurrent public RPC service. An 8192-byte Rust-owned buffer accommodates the
bounded 2048-code-unit exception summary. Error text is not a full exception
object, traceback, or inner-exception chain. Actual plugin failures will require
loader-stage context as that loader is implemented.

## Static evidence and remaining work

| Identity | Value |
| --- | --- |
| Original ACT MVID | 94f61cd2-1f88-4a7c-98d1-0646897b1fb4 |
| REA Dnum evidence | ev_a9d84025e94d05ec20a96cc363c1a5ba1a4e7ba4ad5b75713fd092a45bf1586a |
| Local retained bundle | binaries/analysis/act-contract-m1-2026-10-20-evidence.json |
| Supplemental IL check | act-contract-m1-selected-il.txt: 14 selected method IL hashes/instruction counts matched REA using the existing PEReader helper |

The next M1 work is the UI-dependent plugin interface/host globals, plugin
resolver, initialization and cleanup. Windows runtime tests, native Linux WinForms
replacement, the original FFXIV plugin, original ACT differential execution, live
capture, aggregation and FF Logs acceptance have not been exercised here.
