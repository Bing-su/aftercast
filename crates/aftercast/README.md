# aftercast

An asynchronous in-process ACT-compatible host for Rust. The crate embeds a
prebuilt .NET 10/C# bridge; consumers supply a trusted .NET 10 installation and
plugin deployment. No SDK, runtime download or plugin sandbox is provided.

```rust,no_run
use aftercast::{extract_bridge, Host, HostConfig};

async fn run() -> Result<(), aftercast::Error> {
    // Extraction is an explicit deployment step; a new directory preserves existing files.
    extract_bridge("/private/app-data/bridge-0.1.0")?;
    let host = Host::start(HostConfig::builder()
        .dotnet_root("/usr/lib/dotnet")
        .data_directory("/private/app-data/state")
        .bridge_directory("/private/app-data/bridge-0.1.0")
        .build()).await?;
    let handle = host.handle();
    assert_ne!(handle.session_id().await?, 0);
    host.shutdown().await
}
```

| Contract     | Behavior                                                                                                                                        |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Execution    | The library starts, uses and stops the CLR session on its dedicated thread; Windows STA and message pumping are internal                        |
| Calls        | `HostHandle` is cloneable, Send + Sync; operations return owned results through futures                                                         |
| Runtime      | Futures use Tokio synchronization primitives but need no library-owned or caller-provided Tokio runtime; examples use Tokio                     |
| Admission    | At most 32 queued commands; excess work and conflicts with active imports/archives/audio retirement return `Error::Busy`                        |
| Cancellation | `start_import(path).await?` returns an `ImportJob` with `progress()`, `cancel()` and `wait().await`; partial history remains after cancellation |
| Shutdown     | `shutdown().await` closes admission, cancels imports, finishes accepted archive I/O and waits for managed/native disposal                       |
| Drop         | Requests shutdown without blocking; use explicit shutdown before removing deployment/state files or immediately restarting                      |
| Failures     | Command errors return directly; consume playback failures with `next_audio_error().await`; 32 unconsumed audio errors cause host failure        |
| Lifetime     | One active session per process; loaded CLR, hostfxr and assemblies remain process-owned after shutdown                                          |

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
