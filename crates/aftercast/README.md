# aftercast

An asynchronous process-isolated ACT-compatible host for Rust. The crate embeds a
prebuilt .NET 10/C# bridge; consumers supply a trusted .NET 10 installation and
plugin deployment. No SDK, runtime download or plugin sandbox is provided.

```rust,no_run
use aftercast::{extract_bridge, Host, HostConfig};

async fn run() -> Result<(), aftercast::Error> {
    // Extraction is an explicit deployment step; a new directory preserves existing files.
    extract_bridge("/private/app-data/bridge-0.1.0")?;
    let host = Host::start(HostConfig::builder()
        .dotnet_root("/usr/lib/dotnet")
        .worker_executable("/private/app-bin/aftercast-worker")
        .data_directory("/private/app-data/state")
        .bridge_directory("/private/app-data/bridge-0.1.0")
        .build()).await?;
    let handle = host.handle();
    assert_ne!(handle.session_id().await?, 0);
    host.shutdown().await
}
```

| Contract     | Behavior                                                                                                                                               |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Execution    | A dedicated worker process owns CLR, plugins, Windows STA, pumping and audio; the parent supervises its lifetime                                       |
| Calls        | `HostHandle` is cloneable, Send + Sync; operations return owned results through futures                                                                |
| Runtime      | Futures use Tokio synchronization primitives but need no library-owned or caller-provided Tokio runtime; examples use Tokio                            |
| Admission    | At most 32 queued commands; excess work and conflicts with active imports/archives return `Error::Busy`                                                |
| Cancellation | `start_import(path).await?` returns an `ImportJob` with `progress()`, `cancel()` and `wait().await`; partial history remains after cancellation        |
| Shutdown     | Closes admission and requests graceful cleanup; after `shutdown_timeout` (default 5 s), kills and reaps the child and returns `Error::ShutdownTimeout` |
| Drop         | Requests shutdown without blocking; use explicit shutdown before removing deployment/state files or immediately restarting                             |
| Failures     | Command errors return directly; consume playback failures with `next_audio_error().await`; 32 unconsumed audio errors cause host failure               |
| Lifetime     | Every Host starts a new process and session; a file lock prevents concurrent ownership of the same state directory                                     |

Build and deploy the matching worker with `cargo build -p aftercast --bin aftercast-worker`
(add `--features audio` for system audio). Supply its explicit path in HostConfig;
examples read it from `AFTERCAST_WORKER`. Startup has a separate 30-second default deadline.
After either graceful shutdown or `ShutdownTimeout`, call `Host::start` with the same
configuration and explicitly recreate/initialize plugins. Restart never silently replays commands.
Forced termination can discard in-memory encounters and interrupt plugin settings writes.
Isolation provides a process lifetime boundary, not a filesystem/network security sandbox.

`set_tts(true)` and `set_sound_player(true)` construct backends in the child;
clear methods disable them. Custom backends use a matching custom executable calling
`run_worker_with_audio`, with factories that execute inside that child.

Create all plugins in a startup batch before initializing any of them, then
initialize in dependency order. Automatic pumping marks startup complete after
all constructed plugins have left the pending state. Initialization status is not
proof that a plugin's private startup has completed.

`import_log(path).await` reads a fixed UTF-8 prefix through the parser and waits
for accepted actions and state restoration. End live combat first. EOF does not
flush a plugin's private deferred events. Query results and bounded original-log
batches own their data; subscription overflow reports exact loss and does not
interrupt aggregation. Subscriptions are observations, not durable recordings.

[Usage](../../docs/usage.md), [architecture](../../docs/architecture.md) and
[development](../../docs/development.md) describe deployment, limits and checks.
The async execution model has Linux fixture coverage and Windows cross-compilation;
Windows runtime, native devices and live game collection require separate validation.
Local pinned FFXIV corrections preserve original files and are version checked.
No files outside this repository are rewritten.

Publication is disabled and the project license is undecided. Bundled .NET
compatibility notices are under `managed/Dotnet.Compatibility*.LICENSE`.
