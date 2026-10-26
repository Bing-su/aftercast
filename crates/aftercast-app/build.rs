// Acquire the target runtime during Cargo builds so bundling needs no installed .NET runtime or shell script.
mod build_support;

use std::{env, fs, path::PathBuf};

use miette::{IntoDiagnostic, Result};

fn main() -> Result<()> {
    println!("cargo::rerun-if-changed=runtime-lock.toml");
    println!("cargo::rerun-if-changed=build_support.rs");
    println!("cargo::rerun-if-env-changed=AFTERCAST_RUNTIME_OFFLINE");
    println!("cargo::rerun-if-env-changed=CARGO_NET_OFFLINE");
    let target = env::var("TARGET").into_diagnostic()?;
    let output =
        PathBuf::from(env::var_os("OUT_DIR").ok_or_else(|| miette::miette!("Missing OUT_DIR"))?);
    let offline = ["AFTERCAST_RUNTIME_OFFLINE", "CARGO_NET_OFFLINE"]
        .iter()
        .any(|name| matches!(env::var(name).as_deref(), Ok("1" | "true")));
    let generated =
        build_support::prepare(include_str!("runtime-lock.toml"), &target, &output, offline)?;
    for archive in generated.archives {
        println!("cargo::rerun-if-changed={}", archive.display());
    }
    fs::write(output.join("runtime.rs"), generated.source).into_diagnostic()?;
    Ok(())
}
