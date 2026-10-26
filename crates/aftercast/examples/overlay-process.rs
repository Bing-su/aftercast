use aftercast::{Host, HostConfig, PluginConfig, extract_bridge};
use camino::Utf8Path;
use serde_json::Value;
use std::io::Write;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if aftercast::run_child_if_requested()? {
        return Ok(());
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run())
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(windows) {
        return Err("the original CEF stack requires Windows".into());
    }
    let mut args = std::env::args().skip(1);
    let runtime = args.next().ok_or("provide the .NET 10 root")?;
    let directory = args
        .next()
        .ok_or("provide a fresh run directory with state/OverlayPluginCef/x64 prepared")?;
    let ffxiv = args
        .next()
        .ok_or("provide the trusted FFXIV plugin assembly")?;
    let overlay = args
        .next()
        .ok_or("provide the trusted OverlayPlugin assembly")?;
    let cactbot = args
        .next()
        .ok_or("provide the trusted CactbotOverlay assembly")?;
    let config = prepare_deployment(&runtime, &directory)?;
    let state = format!("{directory}/state");
    let mut previous_pid = 0;
    // Repeat the rendered checks after a real worker exit using the same persisted state and CEF deployment.
    for pass in 1..=2 {
        let host = Host::start(config.clone()).await?;
        let pid = host.process_id();
        assert_ne!(pid, std::process::id());
        assert_ne!(pid, previous_pid);
        for (path, name) in [
            (&ffxiv, "FFXIV_ACT_Plugin.FFXIV_ACT_Plugin"),
            (&overlay, "RainbowMage.OverlayPlugin.PluginLoader"),
            (&cactbot, "Cactbot.PluginLoader"),
        ] {
            let plugin = host
                .create_plugin(
                    PluginConfig::builder()
                        .path(path.as_str())
                        .type_name(name)
                        .build(),
                )
                .await?;
            println!("pass {pass}: {}", host.initialize_plugin(plugin).await?);
        }
        // Allow vendor phase two, then run the existing bounded full-flow assertions inside the actual child.
        tokio::time::sleep(Duration::from_secs(12)).await;
        let probe = tokio::time::timeout(
            Duration::from_secs(90),
            host.create_plugin(
                PluginConfig::builder()
                    .path(format!("{directory}/probe/Aftercast.ManagedTests.dll"))
                    .type_name("OverlayProcessProbe")
                    .build(),
            ),
        )
        .await??;
        println!("pass {pass}: {}", host.initialize_plugin(probe).await?);
        let read_json = |name: &str| -> Result<Value, Box<dyn std::error::Error>> {
            Ok(serde_json::from_slice(&std::fs::read(format!(
                "{state}/{name}"
            ))?)?)
        };
        let started = read_json("process-probe-start.json")?;
        assert_eq!(started.get("pid"), Some(&Value::from(pid)));
        assert_eq!(started.get("apartment"), Some(&Value::from("STA")));
        let result = read_json("process-probe-result.json")?;
        assert_eq!(result.get("pid"), Some(&Value::from(pid)));
        assert_eq!(result.get("passed"), Some(&Value::Bool(true)));
        assert_eq!(host.diagnostics().await?.total, 0);
        host.shutdown().await?;
        let teardown = read_json("process-teardown.json")?;
        assert_eq!(teardown.get("pid"), Some(&Value::from(pid)));
        assert_eq!(
            teardown.get("cactbotLogHookRemoved"),
            Some(&Value::Bool(true))
        );
        save_evidence(&state, &format!("{directory}/run-{pass}"))?;
        println!(
            "pass {pass}: parent {}, worker {pid}, CEF/Edge rendered assertions passed; zero diagnostics; shutdown and hook cleanup passed",
            std::process::id()
        );
        previous_pid = pid;
    }
    Ok(())
}

fn prepare_deployment(
    runtime: &str,
    directory: &str,
) -> Result<HostConfig, Box<dyn std::error::Error>> {
    let bridge = format!("{directory}/bridge");
    extract_bridge(bridge.as_str())?;
    let state = format!("{directory}/state");
    std::fs::create_dir_all(format!("{state}/Config"))?;
    std::fs::create_dir_all(format!("{directory}/probe"))?;
    // Stage only the test assembly so its resolver reuses the child bridge rather than loading another copy.
    std::fs::copy(
        Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../managed/tests/ManagedTests/bin/Release/net10.0/Aftercast.ManagedTests.dll",
        ),
        format!("{directory}/probe/Aftercast.ManagedTests.dll"),
    )?;
    // Prepare only fresh validation state; never replace an existing user's OverlayPlugin settings.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(format!(
            "{state}/Config/RainbowMage.OverlayPlugin.config.json"
        ))?
        .write_all(br#"{"Overlays":[],"UpdateCheck":false,"WSServerRunning":false}"#)?;
    Ok(HostConfig::builder()
        .dotnet_root(runtime)
        .bridge_directory(bridge)
        .data_directory(state)
        .build())
}

fn save_evidence(state: &str, output: &str) -> Result<(), std::io::Error> {
    // Keep both lifetimes' screenshots and assertions before the next run updates its state files.
    std::fs::create_dir_all(format!("{output}/browser"))?;
    for name in [
        "process-probe-start.json",
        "process-probe-result.json",
        "process-teardown.json",
        "cef-raidboss.png",
        "cef-report.json",
        "cef-audio-report.json",
        "overlay-integration.log",
        "tts-route.json",
        "sound-route.json",
        "browser/browser-report.json",
        "browser/raidboss.png",
    ] {
        std::fs::copy(format!("{state}/{name}"), format!("{output}/{name}"))?;
    }
    Ok(())
}
