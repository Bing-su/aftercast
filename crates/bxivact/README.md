# bxivact

An in-process ACT-compatible host for Rust. Implements the .NET 10/F# bridge
lifecycle, initial ACT contracts, and trusted plugin construction/initialization.
Fixture lifecycles work on Linux; original FFXIV initialization and parsing remain incomplete.

Consumers need an installed .NET 10 runtime, but no .NET SDK. The prebuilt bridge
is embedded in the crate. Runtime installation and downloads are never automatic.

```rust,no_run
use bxivact::{extract_bridge, Host, HostConfig};

# fn main() -> Result<(), bxivact::Error> {
extract_bridge("/private/app-data/bridge-0.1.0")?;
let host = Host::start(HostConfig::builder()
    .dotnet_root("/usr/share/dotnet")
    .data_directory("/private/app-data/state")
    .bridge_directory("/private/app-data/bridge-0.1.0")
    .build())?;
assert_ne!(host.session_id()?, 0);
host.stop()?;
# Ok(())
# }
```

Paths use camino UTF-8 types; strings work directly. Convert existing PathBuf values
with `camino::Utf8PathBuf::try_from`. Use a new extraction directory with an existing,
application-controlled parent.
Keep deployed files while the process uses the CLR. Only one host is active at a
time, and the host stays on its creation thread. Stopping it does not unload .NET.
Use `Host::create_plugin(PluginConfig::builder().path(path).build())` to construct
the only eligible plugin type, or select it with `type_name("Namespace.Plugin")`.
Constructors execute in this process; instances are retained until the host stops.
Call `initialize_plugin(id)` for bounded status text and `deinitialize_plugin(id)`
for explicit cleanup; stop also cleans up initialized plugins. Linux supports
headless status controls, with no rendering or native handles. Windows requires
the .NET 10 Desktop Runtime and a caller-owned STA/UI loop; execution is unverified.

The project license is undecided; publication is disabled. The bundled FSharp.Core
10.0.113 dependency is MIT-licensed; its notice is in `managed/FSharp.Core.LICENSE`.
System.Security.Permissions and System.Windows.Extensions 10.0.13 provide legacy
binding support; their MIT notice and upstream notices are bundled as
`managed/Dotnet.Compatibility*.LICENSE`. These packages do not provide a sandbox.
