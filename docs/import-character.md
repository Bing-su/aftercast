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

## Bridge / Rust checkpoint

`ImportCharacter::new(name)` owns a validated, control-free XML string of at most
1024 UTF-8 bytes; `ImportCharacter::current()` is the explicit empty choice.
`SourceImportOptions.viewpoint: None` retains the existing exact programmatic
identity. `Some(choice)` applies the managed ACT UI rules. Rust does not trim or
pre-normalize UTF-16 length. Whole/range imports, decoder choices and companions
share the existing owned source-import lifecycle, with selection copied at Begin.
Live opens, catalog/continuation commands and history captures reject overrides.

`start_clipboard_import_with_viewpoint(text, choice)` accepts already supplied
Unicode text without reading the OS clipboard. Legacy `start_clipboard_import`
and `import_clipboard_text` continue to omit the choice. Binary command 6 encodes
a separate 0..1024-byte UTF-8 name prefix before the 1..8MiB text payload; command
5 retains its old layout and independent text budget. Both are validated before
admission, borrow no native memory, and reuse existing Advance/cancel/drain.
Malformed names, lengths, reserved fields, owner and continuation fields fail
without starting a job. No new capture override or live-file policy is introduced.

`import_character` runs actual isolated-worker synthetic tests for file/range,
codec and companion choices, caller option copies, Unicode self aliases,
omitted short-name versus explicit-current fallback, cancellation/partial
failure/retry restoration and saved identity metadata across worker restart.
Managed ABI checks cover malformed name prefixes and import-only option use;
Rust packet tests cover the simultaneous 8MiB text + 1024-byte name boundary.
These are contract/synthetic execution evidence, not native ACT UI execution.

## Desktop GUI checkpoint

The Import / Export pane uses the existing Rust/UI-derived `Button`, `Input`
and `Label` components in `import_text::character::Controls`, inside the current
Dioxus/Tailwind/Catppuccin layout. No new widget library or HTML/JavaScript bridge
is added. Korean and English labels describe these three separate policies:

| Choice                                  | Requested policy                                                                                                                                       |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Preserve current name exactly (default) | Omitted API option; keep the existing programmatic current name, including a short name.                                                               |
| Current name (ACT rules)                | Explicit empty choice; resolve current name at worker Begin and apply ACT's short-name fallback.                                                       |
| Enter a name                            | Preserve entered text, including whitespace; preview the UTF-16 short-name fallback to YOU. An empty manual entry is the same explicit-current policy. |

The editable next-import choice and admitted request policy have separate labels.
The display does not claim to look up a live current name or predict a plugin's
later intentional mutation of ActGlobals. Full-file selection copies the choice
before the file picker, while range/text admission copies their choice before
starting the async job. Every production request dispatcher uses that copy,
including selected companions and codecs. Controls reject edits during an active
read/import; mode changes, import completion/cancel and Host replacement invalidate
old rendered tokens. Tab navigation keeps admitted work app-owned until drain.

Invalid manual input stays editable and visible, with a localized error and
raw-import admission disabled. Cancel, file/admission failure and parser failure
keep the draft. Host Stop/restart keeps it in the running app; closing/reopening
the app resets to the omitted-option default. No viewpoint preference is written
to config or source files. History capture replay has no manual-choice field and
continues to use its saved exact character; changing a raw-import draft does not
alter or block that capture policy.

Four new actual isolated Host/controller tests cover full-file choices and
picker cancel, forced independent draft versus admitted snapshot, invalid-name
blocking, missing-file recovery, range/codec bounds, companion selection,
provided text via an in-memory reader, partial errors, callback-gated cancel,
duplicate requests, navigation, stale replies/edits and app/Host reset behavior.
The existing history-source controller regression additionally replays a saved
Source viewpoint with a distinct Target raw-import draft and verifies the saved
aggregate/identity remains unchanged. No OS clipboard is read or written.

## Verification limits

The Windows WebView2 suite includes a viewpoint scenario for mode buttons,
manual draft, import results, validation, fallback, Host Stop and app restart.
On Linux it is collected/parsed only; it is not native click evidence. Native ACT
progress UI, Windows focus/input and live game/parser integration remain
unverified. ser7's existing native UI automation/accessibility limitation is not
changed. Build/startup smoke and synthetic controllers do not establish visual,
focus, keyboard or platform clipboard behavior.

L6b OS provider code/wiring remains independently on hold after automatic
approval review rejected it, pending the user's direct approval. This work adds
no provider, retries no rejected operation and accesses only synthetic supplied
text. The L6c option is wired to the supplied-text GUI path; the default build's
OS Paste action remains unavailable under that separate L6b blocker.
