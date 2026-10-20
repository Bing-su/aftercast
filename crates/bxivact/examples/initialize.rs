use bxivact::{Host, HostConfig, LogSubscriptionConfig, PluginConfig, extract_bridge};
use camino::Utf8PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = Utf8PathBuf::try_from(args.next().ok_or("provide a .NET 10 root")?)?;
    let directory = Utf8PathBuf::try_from(args.next().ok_or("provide a new bridge directory")?)?;
    let path = Utf8PathBuf::try_from(args.next().ok_or("provide a trusted plugin assembly")?)?;
    let name = args
        .next()
        .ok_or("provide the fully qualified plugin type")?
        .into_string()
        .map_err(|_invalid| "plugin type must be UTF-8")?;
    let import = args.next().map(Utf8PathBuf::try_from).transpose()?;
    extract_bridge(&directory)?;
    let mut host = Host::start(
        HostConfig::builder()
            .dotnet_root(root)
            .data_directory(directory.join("state"))
            .bridge_directory(directory)
            .build(),
    )?;
    let plugin = host.create_plugin(PluginConfig::builder().path(path).type_name(name).build())?;
    println!(
        "InitPlugin returned status: {}",
        host.initialize_plugin(plugin)?
    );
    // Keep servicing owner-thread callbacks during worker startup; applications do this in their main loop.
    let until = std::time::Instant::now() + std::time::Duration::from_millis(1500);
    while std::time::Instant::now() < until {
        host.pump()?;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if let Some(path) = import {
        let subscription = host.subscribe_logs(LogSubscriptionConfig::builder().build())?;
        println!("Import summary: {:?}", host.import_log(path)?);
        if let Some(snapshot) = host.combat_snapshot(None)? {
            println!("Latest combat result: {snapshot:?}");
        }
        let mut delivered = 0;
        let mut omitted = 0;
        loop {
            let batch = host.drain_logs(subscription)?;
            delivered += batch.records.len();
            omitted += batch.dropped.map_or(0, |loss| loss.count);
            if batch.records.is_empty() {
                break;
            }
        }
        println!("Original dispatched records delivered: {delivered}; omitted: {omitted}");
        host.unsubscribe_logs(subscription)?;
    }
    println!("Worker diagnostics: {:?}", host.diagnostics()?);
    host.stop()?;
    Ok(())
}
