# Usage and current support

The crate embeds the managed assemblies and hosts them in a supplied .NET 10
installation. See the [glossary](../GLOSSARY.md) for domain terms.

## Deployment and host lifetime

```rust,no_run
use aftercast::{Host, HostConfig, extract_bridge};

fn main() -> Result<(), aftercast::Error> {
    // Deploy to a new private directory so existing files are never overwritten.
    extract_bridge("/private/app-data/bridge-0.1.0")?;
    let host = Host::start(HostConfig::builder()
        .dotnet_root("/usr/lib/dotnet")
        .bridge_directory("/private/app-data/bridge-0.1.0")
        .data_directory("/private/app-data/state")
        .build())?;
    assert_ne!(host.session_id()?, 0);
    host.stop()?;
    Ok(())
}
```

| Concern        | Contract                                                                                                   |
| -------------- | ---------------------------------------------------------------------------------------------------------- |
| Runtime        | Supply a trusted installed .NET 10 root; the library does not select or download an unrelated installation |
| Paths          | UTF-8 camino paths; string callers work directly, PathBuf callers convert explicitly                       |
| Extraction     | Parent exists and is application-controlled; destination must be new; incomplete extraction rolls back     |
| State          | A private writable directory separate from deployed binaries; existing contents are preserved              |
| Lifetime       | One active Host per process; it is neither Send nor Sync and stays on its creation thread                  |
| Shutdown       | Explicit stop reports cleanup failures; Drop attempts cleanup and logs failures                            |
| Restart        | New ACT session identity; CLR, hostfxr and loaded assemblies remain process-owned                          |
| Runtime switch | A different loaded hostfxr installation requires a new process                                             |

## Plugins and owner callbacks

| Operation                     | Meaning                                                                                                                                                                    |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `create_plugin(PluginConfig)` | Prepare ACT globals/settings directory, then construct a trusted IActPluginV1; Windows requires caller-owned STA; select type_name when more than one eligible type exists |
| `initialize_plugin(id)`       | Attempt InitPlugin once; return bounded status text; callback return does not prove parser readiness                                                                       |
| `pump()`                      | Run owner-thread callbacks and Windows native messages; call regularly from the application's loop                                                                         |
| `diagnostics()`               | Read bounded host diagnostic history without consuming it; inspect total and omitted()                                                                                     |
| `deinitialize_plugin(id)`     | DeInitPlugin at most once after successful initialization; unregister and dispose owned controls                                                                           |
| `stop()`                      | Attempt cleanup of all initialized plugins before releasing the session                                                                                                    |

Plugin constructors and callbacks execute trusted code in the consumer process.
There is no sandbox or assembly unloading. Failed initialization cannot be retried
on the same instance, and arbitrary plugin side effects cannot be rolled back.
Session-scoped plugin IDs, subscriptions and revisions cannot be reused after restart.
The [initialize example](../crates/aftercast/examples/initialize.rs) demonstrates
pumping, import and owned output through the public interface.

| Deferred integration contract | Usage                                                                                                                                                                                                      |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `InitActDone`                 | Starts false. A pump marks startup ready after all created instances leave constructed/initializing state and a plugin has initialized. Creating another instance reopens the boundary; shutdown clears it |
| Startup batch                 | Initialize FFXIV, OverlayPlugin and cactbot in that order before the first pump; then pump regularly until OverlayPlugin reports Initialised. This lets its deferred addon scan see the full batch         |
| Plugin metadata               | ACT's active checkbox and title/status controls are available to registered plugins and remain owned by the host                                                                                           |
| Shutdown                      | DeInit runs before owned controls and optional IDisposable cleanup. OverlayPlugin event sources are stopped before their container is disposed so addon timers/hooks do not outlive the embedded host      |

## Input and owned output

ACT TTS is opt-in: add the standalone `aftercast-tts` crate and call
`host.set_tts(aftercast_tts::Speaker::system()?)?` before loading plugins. Regular
`host.pump()` delivers requests and reports eventual playback failures;
`host.clear_tts()` cancels speech and disables the route. No online fallback is
automatic. See [TTS setup and limits](tts-validation.md).

Effects are opt-in independently. Enable `aftercast`'s `audio` feature (or select
`system`/`playback` on your `aftercast-tts` dependency), then configure before loading plugins:

```rust,ignore
host.set_tts(aftercast_tts::Speaker::system()?)?;
host.set_sound_player(aftercast_tts::SoundPlayer::system()?)?;
host.set_audio_settings(&aftercast::AudioSettings {
    sound_volume: 60,
    tts_volume: 80,
    timer_expire: "tts expired".into(),
    ..Default::default()
})?;
```

Pump regularly to dispatch queued requests and observe playback errors. Replacing
or clearing an output cancels its active and queued work; settings last for one
session. See [sound/timer contracts and platform validation](audio-validation.md).

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
| Controls and charts                                          | Headless metadata, ownership and explicit event delivery                                  | Native controls require .NET 10 Desktop Runtime and caller-owned STA; charts are unverified                                      |
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
