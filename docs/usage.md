# Usage and current support

The Rust interface owns execution and exposes futures. A trusted .NET 10 runtime
and plugins run in a supervised child process; the library supplies its own host thread and message
loop. See the [glossary](../GLOSSARY.md) for domain terms.

## Deployment and host lifetime

```rust,no_run
use aftercast::{Host, HostConfig, extract_bridge};

async fn run() -> Result<(), aftercast::Error> {
    // Deploy before starting the host; existing destinations are never overwritten.
    extract_bridge("/private/app-data/bridge-0.1.0")?;
    let host = Host::start(HostConfig::builder()
        .dotnet_root("/usr/lib/dotnet")
        .worker_executable("/private/app-bin/aftercast-worker")
        .bridge_directory("/private/app-data/bridge-0.1.0")
        .data_directory("/private/app-data/state")
        .build()).await?;
    assert_ne!(host.session_id().await?, 0);
    host.shutdown().await
}
```

| Concern        | Contract                                                                                                                                          |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| Runtime        | Supply an installed trusted .NET 10 root; Windows needs the Desktop Runtime for native WinForms                                                   |
| Paths          | UTF-8 camino paths; extraction is a synchronous deployment operation; command path resolution runs on the host thread                             |
| State          | Private writable directory, separate from deployed binaries; existing contents are preserved                                                      |
| Ownership      | `Host` owns the session lifetime; cloneable `HostHandle` is Send + Sync; the library owns Windows STA                                             |
| Scheduling     | Automatic bounded command processing, callbacks, native messages, timers and audio; no public `pump()`                                            |
| Admission      | 32 queued commands; full queues and conflicting active operations return `Error::Busy`                                                            |
| Response loss  | Dropping an admitted command's future does not undo or cancel its mutation                                                                        |
| Shutdown       | Default 5 s graceful deadline; expiry kills/reaps the child and returns `Error::ShutdownTimeout`; Drop requests the same shutdown without waiting |
| Restart        | Await shutdown before immediately restarting; old IDs and handles cannot access a new session                                                     |
| Runtime switch | Each Host has a fresh worker, so subsequent hosts can select another runtime deployment                                                           |

The futures do not require a Tokio runtime: the library uses synchronization
primitives, while its host thread drives native work. Examples use a Tokio
executor. Plugin callbacks can still block the child thread; an independent parent
supervisor enforces shutdown without calling plugin code. `startup_timeout` defaults
to 30 s and `shutdown_timeout` to 5 s. Both must be positive. Explicit shutdown
returns only after the direct child has exited and been reaped. Forced termination
does not guarantee settings flush, accepted-operation completion or import restoration.
Await shutdown (including a ShutdownTimeout result) before restarting with the same
state directory; concurrent owners receive `Error::StateInUse`. Recreate plugins
explicitly in the new Host. The worker is a trusted process, not a security sandbox.

Build `aftercast-worker` from the same version (`cargo build -p aftercast --bin aftercast-worker`),
deploy it alongside the application and pass its explicit path. No PATH search or
runtime compilation is performed. Set `AFTERCAST_WORKER` for the supplied examples.

## Plugins and owner callbacks

| Operation                       | Meaning                                                                                           |
| ------------------------------- | ------------------------------------------------------------------------------------------------- |
| `create_plugin(config).await`   | Construct a trusted plugin on the host thread with ACT globals and settings ready                 |
| `initialize_plugin(id).await`   | Attempt InitPlugin once and return bounded status text; no guarantee of private parser readiness  |
| `deinitialize_plugin(id).await` | DeInitPlugin at most once, remove registrations and dispose controls                              |
| `diagnostics().await`           | Copy bounded diagnostic history; compare total and omitted counts                                 |
| `shutdown().await`              | Close admission, restore an active import, settle accepted archive work and dispose plugins/audio |

Create **all** plugins in a startup batch first, then initialize FFXIV,
OverlayPlugin and cactbot in dependency order. Constructed instances keep
`InitActDone` false until every pending instance has finished initialization.
Creating another instance reopens that startup state. Native controls remain on
the internal STA thread; they cannot be accessed directly from a GUI executor.

Constructors and callbacks are trusted code inside the worker. Initialization cannot be
retried on a failed instance, and arbitrary side effects cannot be rolled back.
External plugins are left unchanged; local source/fixture plugins and versioned
local compatibility transformations remain repository-owned.

## Input and owned output

All host operations in the tables below are awaited. For cancellation/progress,
use `start_import(path).await?`, inspect the returned job's `progress()`, request
`cancel()`, and await `wait()` for restored live state. `import_log(path).await`
is the convenience operation without an exposed job. A dropped job keeps running.
Imports and archives exclude conflicting mutations; diagnostic and owned graph/log
queries remain available between processing steps.

Audio is opt-in and speech/effects have independent workers inside the child.
Build the worker with `--features audio`, then await `set_tts(true)`,
`set_sound_player(true)` and `set_audio_settings`. Custom worker executables can
call `run_worker_with_audio` to supply their own backend factories; live backend
objects are never transferred from the parent. Consume playback failures through the owning
`Host::next_audio_error().await`; command acceptance does not establish playback
success. Its error queue holds 32 entries; overflow fails the host visibly.
Replacing or clearing a route cancels and disposes its prior worker before the
command completes. No online fallback is automatic.

Remote cactbot browsers use their own HTMLAudio and Web Speech engines. They need
an installed local voice matching cactbot's display language and an audio-unlock
click; Linux Chromium also needs Speech Dispatcher enabled. See the verified
[browser setup and playback checks](browser-audio-validation.md).

| Operation                                                       | Meaning and limits                                                                                             | Detail                                         |
| --------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| `end_combat()`                                                  | Quiesce live delivery, drain admitted actions and publish the latest ended result; idle calls are harmless     | [Combat snapshots](snapshots.md)               |
| `import_log(path)`                                              | Import the initially opened UTF-8 file prefix through initialized plugins; call while live combat is idle      | [File import](import.md)                       |
| `subscribe_logs(config)`                                        | One bounded original-text subscription per session; observation precedes mutable parser hooks                  | [Log batches](logs.md)                         |
| `drain_logs(id)`                                                | Return owned records and exact oldest-record loss ranges; core parsing continues during subscriber overflow    | [Log batches](logs.md)                         |
| `unsubscribe_logs(id)`                                          | Discard unread subscription records and loss state explicitly                                                  | [Log batches](logs.md)                         |
| `combat_snapshot(after)`                                        | Latest ended encounter and import status, with session-scoped revision polling; intermediate versions coalesce | [Combat snapshots](snapshots.md)               |
| `live_combat()`                                                 | Current live encounter and participant counters without ending combat                                          | [GUI queries](snapshots.md#gui-combat-queries) |
| `encounters(offset, limit)`                                     | Paged session encounter list, including imports and active combat                                              | [GUI queries](snapshots.md#gui-combat-queries) |
| `encounter_detail(id)` / `encounter_attacks(id, offset, limit)` | Selected encounter participants and paged category/attack aggregates                                           | [GUI queries](snapshots.md#gui-combat-queries) |

Import errors restore live parser context and retain any partial imported history.
Transport counts are not parsed counts. End-of-file does not flush private plugin
deferred events. The original plugin may retain private state between imports;
a new import is not a promise of fresh plugin state. Inspect host and plugin
diagnostics and the snapshot's PartialImport status.

Retained records and snapshots own their data after subsequent calls and host
shutdown. Subscriber loss and superseded snapshot versions have different meanings:
one reports omitted observations, the other reports replaced versions of a result.
Neither output is a durable recording or complete encounter archive.

For explicit encounter persistence, ACT XML/gzip interchange and restart search,
use [encounter archives and history](encounter-archives.md): `save_history()`,
`restore_history(filter)`, `import_act(path)` and `export_act(id, path, format)`.
Checkpointing is explicit; ordinary shutdown does not automatically save history.

## Data corrections

Prepare `DataCorrections.xml` in the private data directory before starting the
host. It is read-only startup configuration; shutdown does not rewrite it.

```xml
<DataCorrections>
  <RenameFix>
    <Rename Before="Pet" After="Owner" Active="True" />
  </RenameFix>
  <AbilityRedirectFix>
    <Redirect Ability="Strike" Into="Player" Type="1" Active="True" />
  </AbilityRedirectFix>
</DataCorrections>
```

| Contract       | Behavior                                                                                                                     |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Names          | Exact, case-sensitive attacker and victim replacement after normalization                                                    |
| Redirects      | Exact ability match replaces the attacker; Type 1 damage swing types, 2 healing types, 3 neither, other values all types     |
| Order          | Renames, then redirects, then BeforeCombatAction, then live timers and aggregation; imported actions receive corrections too |
| Multiple rules | ACT's sorted display-key order, not XML order; later matching rules see earlier edits                                        |
| Duplicates     | Rename duplicates retain the first rule; redirect duplicates update only its type, preserving original Active state          |
| Validation     | DTDs are prohibited; malformed XML, missing required attributes and invalid numbers fail startup; 4 MiB character limit      |

This implements the headless rename/redirect path, not every ACT correction
option or its settings editor. Selective `EncounterData` constructors also honor
the host's SelectiveList; ordinary host-created encounters keep their existing
nonselective policy.

## Custom triggers

The host loads `CustomTriggers.xml` from its private data directory before plugin
construction and saves definitions on orderly shutdown. Plugins can use
`CustomTriggers`, `ActiveCustomTriggers`, `AddEditCustomTrigger` and
`RebuildActiveCustomTriggers`. A consumer can also prepare the file before startup:

```xml
<CustomTriggers>
  <Trigger Regex="(?&lt;YOU&gt;Source) prepares (?&lt;ability&gt;.+)" Category="Fixture"
           Active="True" CategoryRestrict="True" SoundType="3" SoundData="${ability}"
           Timer="False" TimerName="" Tabbed="False" />
</CustomTriggers>
```

| Contract    | Behavior                                                                                                                                                                                |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Input       | Live followed log files, after parser hooks; `OriginalLoglineCustomTriggers=true` selects the original line                                                                             |
| Matching    | Compiled .NET regex, first match per trigger/line, case sensitive; named `YOU` capture must contain the configured character name, ignoring case                                        |
| Categories  | Active triggers run everywhere unless CategoryRestrict is enabled; then the current zone must contain the category, ignoring case                                                       |
| Audio       | SoundType 0 silent, 1 beep, 2 local sound (missing file falls back to beep), 3 TTS with .NET match substitutions such as `${ability}`; one-second per-trigger audio suppression         |
| Timers      | TimerName routes to a registered spell timer with attacker/victim captures and all regex captures; timer notification occurs before audio suppression                                   |
| Imports     | No trigger audio or live timer side effects; tabbed result collection/rendering is not implemented                                                                                      |
| Persistence | ACT-style CustomTriggers/Trigger XML; 4 MiB maximum; malformed XML/regex fails startup without overwriting the file; complete settings replace the previous file atomically on shutdown |
| Execution   | Synchronous on the log reader, 100 ms timeout per regex; failures are diagnosed and later triggers continue. This does not reproduce ACT's separate trigger queue scheduling            |

Sound and speech still require the explicit Rust output setup above. Definitions
are not saved after a process crash. Avoid editing the file while a host owns it;
shutdown saves the in-memory registry. This persists trigger settings, not combat
history or a durable log recording.

## Platform support

| Area                                                         | Linux                                                                                     | Windows                                                                                                                          |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| CLR hosting and fixture plugins                              | Executed in integration tests                                                             | Executed in integration tests                                                                                                    |
| Controls and charts                                          | Headless metadata, ownership and explicit event delivery                                  | Native controls require .NET 10 Desktop Runtime; the library owns STA; charts are unverified                                     |
| Rendering/native handles                                     | Explicitly unsupported                                                                    | Native handles and Invoke tested; visible rendering unverified                                                                   |
| Live log following                                           | Append, split UTF-8, truncation, same-path replacement and initial partial records tested | Same file-following contracts and fixture live snapshots tested                                                                  |
| Original FFXIV plugin                                        | Prior synthetic and recorded Zeromus observations                                         | 3.0.3.4 import/lifecycle results recorded in the [Windows report](windows-original-validation.md); full compatibility unverified |
| Native game capture / durable recording / FF Logs acceptance | Unverified                                                                                | Unverified                                                                                                                       |

The pinned FFXIV 3.0.3.3 Linux deployment receives three in-memory corrections:
administrator probing, writer cancellation disposal, and resource CRLF delimiter
semantics. Corrections apply only to recognized artifact hashes; original files
remain unchanged.
These corrections do not drain the vendor writer's pending queue or establish
recording durability. See [compatibility](compatibility.md) for the affected
artifacts and remaining limits.
