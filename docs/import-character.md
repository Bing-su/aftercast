# Import character / point of view

## ACT reference

The pinned ACT 3.8.5.288 artifact has SHA-256
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
The `OriginalReference import-policy` probe reads its actual selection/setter IL;
it does not construct the original UI or run its import thread.

| Original location                                       | Verified behavior                                                                                                                                                |
| ------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| FormImportProgress InitializeComponent                  | Labels the choice “Select a Point of View” and “Character Name”                                                                                                  |
| btnLastKnownName_Click / btnOk_Click IL_0002–IL_0017    | Copies button/textbox Text to charName and signals waitCharName                                                                                                  |
| ImportFile IL_0013–IL_007b                              | Only userInterfaceDriven imports wait for this choice and call SetCharName(false). Programmatic false bypasses that UI normalization.                            |
| ImportFile IL_0026–IL_006c / ImportClip IL_0023–IL_0069 | Nonempty choice replaces the character field; empty choice uses current ActGlobals.charName. Both UI paths apply SetCharName(false), without filename discovery. |
| SetCharName IL_000c–IL_0036                             | Uses the selected string verbatim when .NET UTF-16 Length > 2; otherwise assigns YOU. It does not trim or normalize case.                                        |

The point of view is character identity, not an actor-display filter. Parsers can
use ActGlobals.charName for self aliases. Encounter.CharName seeds GetAllies and
therefore allied damage/healing and enemy-title selection. Choosing an absent
actor can yield zero allied totals while retaining every accepted actor/action.
No new selective-parsing mode or game-specific character classifier is added.

## Managed checkpoint

`ImportCharacterOptions(Name)` is an immutable, optional explicit choice on
HostEnvironment ImportLog/BeginImport/BeginClipboardImport. A nonempty entered
name follows ACT's UTF-16 length fallback. An explicit empty choice uses the
current name and then the same fallback. Entered names are not trimmed. Actor
normalization still trims action names as before, so leading/trailing whitespace
in a character choice can leave no matching ally seed.

| Admission/lifetime | Contract                                                                                                                                                                                                                                                                                                                                                                           |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Omitted options    | Preserve the existing programmatic current character exactly, even a short name. This corresponds to ACT ImportFile(userInterfaceDriven=false), rather than implicitly applying new UI normalization to legacy callers.                                                                                                                                                            |
| Explicit choice    | Validate before state mutation: non-null, no control characters, valid XML text/well-formed Unicode, at most 1024 UTF-8 bytes. Empty is allowed as the ACT UI's current-name choice. This fixed entered-name budget is aftercast's bound, not an original ACT limit; it does not rewrite inherited/saved names.                                                                    |
| Snapshot           | Resolve/copy the effective character at managed Begin, before log-reader suspension. Later caller option copies or live global changes before preparation do not change that admitted identity. Apply it before import callbacks and new encounters. ACT's global remains plugin-mutable; this option does not prevent a plugin deliberately changing it during its own callbacks. |
| Live restoration   | Existing LiveParserContext captures the live state at preparation and restores its exact character/path/zone/time after success, failure or cancel. A legitimate live change between Begin and preparation is restored, while the import uses its earlier snapshot.                                                                                                                |
| Inputs             | One identity applies to main and companion records, whole/range input, and explicitly supplied Unicode text. No OS clipboard access is performed.                                                                                                                                                                                                                                  |
| Private history    | Saved encounter character/ally graph and source sorter links survive restore. One-shot captured history continues to use its nonempty saved exact character, independently of manual raw-import choices; a saved two-unit character is not rewritten to YOU. Empty/missing saved hints inherit current identity as before.                                                         |
| File discovery     | No character is inferred from a filename, directory or game installation. Existing unsupported automatic filename-discovery behavior is unchanged.                                                                                                                                                                                                                                 |

`ImportCharacterChecks` uses only synthetic logs and temporary archives. It
checks Source versus Target aggregates/title, unmatched viewpoints with retained
actors, parser self aliases, Korean/Japanese/emoji UTF-16 fallback, untrimmed
names, the omitted-versus-explicit-current distinction, mutable caller/live
changes after Begin, input limits and invalid Unicode/control/XML characters,
failure/cancel/missing-file restoration, companions, supplied text, private
restart and exact saved-character capture replay. The ordinary managed contracts
run includes these checks.

## Remaining L6c work

1. Add bounded Bridge/Rust request-owned character selection without changing
   default file/range/text callers or allowing overrides of captured history.
2. Reuse existing Rust/UI controls for current/manual choice, explaining effective
   selection and keeping draft versus admitted context distinct; test actual
   isolated Host/controller behavior, stale sessions, failure/cancel and restart.
3. Native ACT progress UI, Windows focus/input and live game/parser integration
   remain unverified. L6b OS provider code/wiring remains independently on hold;
   this checkpoint does not retry or bypass that approval blocker.
