# bxivact

An in-process ACT-compatible host for Rust. M0 implements the .NET 10/F# bridge
lifecycle only; FFXIV plugin loading and parsing are not implemented yet.

Consumers need an installed .NET 10 runtime, but no .NET SDK. The prebuilt bridge
is embedded in the crate. Runtime installation and downloads are never automatic.

```rust,no_run
use bxivact::{extract_bridge, Host, HostConfig};

# fn main() -> Result<(), bxivact::Error> {
extract_bridge("/private/app-data/bridge-0.1.0")?;
let host = Host::start(HostConfig::builder()
    .dotnet_root("/usr/share/dotnet")
    .bridge_directory("/private/app-data/bridge-0.1.0")
    .build())?;
assert_ne!(host.session_id()?, 0);
host.stop()?;
# Ok(())
# }
```

Use a new extraction directory with an existing, application-controlled parent.
Keep deployed files while the process uses the CLR. Only one host is active at a
time, and the host stays on its creation thread. Stopping it does not unload .NET.

The project license is undecided; publication is disabled. The bundled FSharp.Core
10.0.113 dependency is MIT-licensed; its notice is in `managed/FSharp.Core.LICENSE`.
