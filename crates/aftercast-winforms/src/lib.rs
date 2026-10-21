//! Prebuilt managed WinForms signature compatibility for Linux hosts.
//!
//! The C# implementation and its contract tests live alongside this crate. No ACT or
//! runtime-hosting dependency is required. Consumers need .NET 10
//! supplied by their host; Cargo never invokes the .NET SDK.
//!
//! A small control hierarchy supports headless status text and disposal. Native handles and
//! rendering throw `PlatformNotSupportedException`; layout and message dispatch are absent.

/// Prebuilt `System.Windows.Forms.dll` for deployment by a Linux .NET host.
///
/// The host owns file deployment and assembly-context selection. For example, aftercast writes
/// these bytes into its private bridge directory and shares the loaded assembly with plugins.
/// Windows hosts must use real WinForms instead of this unsigned compatibility assembly.
pub const ASSEMBLY: &[u8] = include_bytes!("../managed/System.Windows.Forms.dll");

/// Prebuilt chart signature assembly; deploy beside [`ASSEMBLY`] in the same load context.
///
/// Chart objects share the headless control lifetime. Data binding and rendering are unsupported;
/// for example `Chart.DataBind` throws rather than fabricating plotted data.
pub const DATAVISUALIZATION_ASSEMBLY: &[u8] =
    include_bytes!("../managed/System.Windows.Forms.DataVisualization.dll");
