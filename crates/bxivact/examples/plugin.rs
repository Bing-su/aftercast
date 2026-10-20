use bxivact::{Host, HostConfig, PluginConfig, extract_bridge};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = args.next().ok_or("provide a .NET 10 installation root")?;
    let directory = args.next().ok_or("provide a new bridge directory")?;
    let path = args
        .next()
        .ok_or("provide a trusted plugin assembly path")?;
    let type_name = args.next();
    extract_bridge(&directory)?;
    let mut host = Host::start(
        HostConfig::builder()
            .dotnet_root(root)
            .bridge_directory(directory)
            .build(),
    )?;
    let id = host.create_plugin(
        PluginConfig::builder()
            .path(path)
            .maybe_type_name(type_name)
            .build(),
    )?;
    println!("Constructed {id:?}; InitPlugin was not called");
    host.stop()?;
    Ok(())
}
