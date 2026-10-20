# bxivact

An in-process ACT-compatible host for Rust. Implements the .NET 10/C# bridge
lifecycle, initial ACT contracts, and trusted plugin construction/initialization.
Original FFXIV initializes and imports a synthetic 400-damage fixture on Linux.
Representative gameplay imports, live capture and Windows execution remain unverified.

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

After initialization, `host.import_log(path)` reads a fixed UTF-8 file prefix and
returns transport counts and actual allied combat totals after submitted actions
drain. End live combat first. Failure restores live context and retains partial
import history. Dispatch counts are not parsed counts, and EOF does not flush
the plugin's private deferred events. Check host and plugin diagnostics.

On Linux, the pinned FFXIV 3.0.3.3 build receives an in-memory compatibility
adjustment: its embedded memory component reports Windows administrator status
as false. Original files remain unchanged; other builds are not rewritten.

The project license is undecided; publication is disabled.
System.Security.Permissions and System.Windows.Extensions 10.0.13 provide legacy
binding support; their MIT notice and upstream notices are bundled as
`managed/Dotnet.Compatibility*.LICENSE`. These packages do not provide a sandbox.
