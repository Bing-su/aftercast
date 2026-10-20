//! Prebuilt managed WinForms signature compatibility for Linux hosts.
//!
//! The F# implementation and its contract tests live alongside this crate. No ACT or
//! runtime-hosting dependency is required. Consumers need .NET 10 and FSharp.Core 10.0.113
//! supplied by their host; Cargo never invokes the .NET SDK.
//!
//! A small control hierarchy supports headless status text and disposal. Native handles and
//! rendering throw `PlatformNotSupportedException`; layout and message dispatch are absent.

/// Prebuilt `System.Windows.Forms.dll` for deployment by a Linux .NET host.
///
/// The host owns file deployment and assembly-context selection. For example, bxivact writes
/// these bytes into its private bridge directory and shares the loaded assembly with plugins.
/// Windows hosts must use real WinForms instead of this unsigned compatibility assembly.
pub const ASSEMBLY: &[u8] = include_bytes!("../managed/System.Windows.Forms.dll");
