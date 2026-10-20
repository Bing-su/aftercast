use bxivact::{Host, HostConfig, extract_bridge};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = args
        .next()
        .or_else(|| std::env::var_os("DOTNET_ROOT"))
        .ok_or("provide a .NET 10 root as the first argument or set DOTNET_ROOT")?;
    let directory = args
        .next()
        .map(std::path::PathBuf::from)
        .ok_or("provide a new bridge extraction directory as the second argument")?;
    extract_bridge(&directory)?;
    let config = HostConfig::builder()
        .dotnet_root(root)
        .bridge_directory(&directory)
        .build();
    let host = Host::start(config)?;
    println!("Managed session {} is ready", host.session_id()?);
    host.stop()?;
    Ok(())
}
