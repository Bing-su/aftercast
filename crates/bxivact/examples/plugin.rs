use bxivact::{Host, HostConfig, PluginConfig, extract_bridge};
use camino::Utf8PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = Utf8PathBuf::try_from(args.next().ok_or("provide a .NET 10 installation root")?)?;
    let directory = Utf8PathBuf::try_from(args.next().ok_or("provide a new bridge directory")?)?;
    let path = Utf8PathBuf::try_from(
        args.next()
            .ok_or("provide a trusted plugin assembly path")?,
    )?;
    let type_name = args
        .next()
        .map(|name| {
            name.into_string()
                .map_err(|_invalid| "plugin type name must be UTF-8")
        })
        .transpose()?;
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
