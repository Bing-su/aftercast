# Usage and current support

The crate embeds the managed assemblies and hosts them in a supplied .NET 10
installation. See the [glossary](../GLOSSARY.md) for domain terms.

## Deployment and host lifetime

```rust,no_run
use bxivact::{Host, HostConfig, extract_bridge};

fn main() -> Result<(), bxivact::Error> {
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

| Operation                     | Meaning                                                                                              |
| ----------------------------- | ---------------------------------------------------------------------------------------------------- |
| `create_plugin(PluginConfig)` | Construct a trusted IActPluginV1; select type_name when more than one eligible type exists           |
| `initialize_plugin(id)`       | Attempt InitPlugin once; return bounded status text; callback return does not prove parser readiness |
| `pump()`                      | Run Linux owner-thread callbacks; call regularly from the application's loop                         |
| `diagnostics()`               | Read bounded host diagnostic history without consuming it; inspect total and omitted()               |
| `deinitialize_plugin(id)`     | DeInitPlugin at most once after successful initialization; unregister and dispose owned controls     |
| `stop()`                      | Attempt cleanup of all initialized plugins before releasing the session                              |

Plugin constructors and callbacks execute trusted code in the consumer process.
There is no sandbox or assembly unloading. Failed initialization cannot be retried
on the same instance, and arbitrary plugin side effects cannot be rolled back.
Session-scoped plugin IDs, subscriptions and revisions cannot be reused after restart.
The [initialize example](../crates/bxivact/examples/initialize.rs) demonstrates
pumping, import and owned output through the public interface.

## Input and owned output

| Operation                | Meaning and limits                                                                                             | Detail                           |
| ------------------------ | -------------------------------------------------------------------------------------------------------------- | -------------------------------- |
| `end_combat()`           | Quiesce live delivery, drain admitted actions and publish the latest ended result; idle calls are harmless     | [Combat snapshots](snapshots.md) |
| `import_log(path)`       | Import the initially opened UTF-8 file prefix through initialized plugins; call while live combat is idle      | [File import](import.md)         |
| `subscribe_logs(config)` | One bounded original-text subscription per session; observation precedes mutable parser hooks                  | [Log batches](logs.md)           |
| `drain_logs(id)`         | Return owned records and exact oldest-record loss ranges; core parsing continues during subscriber overflow    | [Log batches](logs.md)           |
| `unsubscribe_logs(id)`   | Discard unread subscription records and loss state explicitly                                                  | [Log batches](logs.md)           |
| `combat_snapshot(after)` | Latest ended encounter and import status, with session-scoped revision polling; intermediate versions coalesce | [Combat snapshots](snapshots.md) |

Import errors restore live parser context and retain any partial imported history.
Transport counts are not parsed counts. End-of-file does not flush private plugin
deferred events. The original plugin may retain private state between imports;
a new import is not a promise of fresh plugin state. Inspect host and plugin
diagnostics and the snapshot's PartialImport status.

Retained records and snapshots own their data after subsequent calls and host
shutdown. Subscriber loss and superseded snapshot versions have different meanings:
one reports omitted observations, the other reports replaced versions of a result.
Neither output is a durable recording or complete encounter archive.

## Platform support

| Area                                                         | Linux                                                                                     | Windows                                                       |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| CLR hosting and fixture plugins                              | Executed in integration tests                                                             | Rust target compilation only                                  |
| Controls and charts                                          | Headless metadata, ownership and explicit event delivery                                  | Requires .NET 10 Desktop Runtime and caller-owned STA/UI loop |
| Rendering/native handles                                     | Explicitly unsupported                                                                    | Actual execution unverified                                   |
| Live log following                                           | Append, split UTF-8, truncation, same-path replacement and initial partial records tested | File identity implementation present; execution unverified    |
| Original FFXIV plugin                                        | Prior synthetic and recorded Zeromus observations                                         | Execution unverified                                          |
| Native game capture / durable recording / FF Logs acceptance | Unverified                                                                                | Unverified                                                    |

The pinned FFXIV 3.0.3.3 Linux deployment receives three in-memory corrections:
administrator probing, writer cancellation disposal, and resource CRLF delimiter
semantics. Corrections apply only to recognized artifact hashes; original files
remain unchanged.
These corrections do not drain the vendor writer's pending queue or establish
recording durability. See [compatibility](compatibility.md) for the affected
artifacts and remaining limits.
