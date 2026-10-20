use std::{env, path::Path};

use bxivact::{Error, Host, HostConfig, extract_bridge};

fn config(root: &Path, bridge: &Path) -> HostConfig {
    HostConfig::builder()
        .dotnet_root(root)
        .bridge_directory(bridge)
        .build()
}

#[test]
fn extraction_preserves_existing_directories() {
    let temporary = tempfile::tempdir().expect("create isolated extraction fixture");
    let directory = temporary.path().join("bridge");
    extract_bridge(&directory).expect("extract bundled bridge");
    #[cfg(target_os = "linux")]
    assert_eq!(
        std::fs::read(directory.join("System.Windows.Forms.dll"))
            .expect("read compatibility assembly"),
        bxivact_winforms::ASSEMBLY
    );
    #[cfg(windows)]
    assert!(!directory.join("System.Windows.Forms.dll").exists());
    let assembly = directory.join("Bxivact.Bridge.dll");
    let expected = std::fs::read(&assembly).expect("read extracted assembly");
    let error = extract_bridge(&directory).expect_err("existing directories must be preserved");
    match error {
        Error::Io { source, .. } => assert_eq!(source.kind(), std::io::ErrorKind::AlreadyExists),
        other => panic!("unexpected extraction error: {other}"),
    }
    assert_eq!(
        std::fs::read(assembly).expect("existing assembly remains"),
        expected
    );
}

#[test]
#[ignore = "requires a .NET 10 installation; set DOTNET_ROOT and run mise run test:hosting"]
fn managed_lifecycle_releases_ownership_after_errors_stop_and_drop() {
    let root =
        env::var_os("DOTNET_ROOT").expect("set DOTNET_ROOT to a trusted .NET 10 installation");
    let root = Path::new(&root);
    let temporary = tempfile::tempdir().expect("create isolated bridge fixture");
    let bridge = temporary.path().join("bridge");
    extract_bridge(&bridge).expect("extract prebuilt managed component");

    let missing_root = temporary.path().join("missing-runtime");
    match Host::start(config(&missing_root, &bridge)) {
        Err(Error::Io { .. }) => (),
        _ => panic!("missing runtime must fail before acquiring a managed session"),
    }

    let invalid_bridge = temporary.path().join("invalid-bridge");
    std::fs::create_dir(&invalid_bridge).expect("create invalid configuration fixture");
    std::fs::write(
        invalid_bridge.join("Bxivact.Bridge.runtimeconfig.json"),
        "not JSON",
    )
    .expect("write invalid runtime configuration");
    match Host::start(config(root, &invalid_bridge)) {
        Err(Error::Hosting(_)) => (),
        _ => panic!("invalid runtime configuration must report a hosting failure"),
    }

    let host = Host::start(config(root, &bridge)).expect("failure releases the host lease");
    let first = host.session_id().expect("managed call succeeds");
    assert_ne!(first, 0);
    match Host::start(config(root, &bridge)) {
        Err(Error::AlreadyActive) => (),
        _ => panic!("second active host must be rejected"),
    }
    host.stop().expect("explicit shutdown succeeds");

    // A loaded CLR cannot move to another installation merely because the ACT session stopped.
    let alternate = temporary.path().join("alternate-runtime");
    let alternate_fxr = alternate.join("host/fxr/10.0.0");
    std::fs::create_dir_all(&alternate_fxr).expect("create alternate runtime fixture");
    let name = if cfg!(windows) {
        "hostfxr.dll"
    } else {
        "libhostfxr.so"
    };
    std::fs::write(alternate_fxr.join(name), []).expect("mark an alternate hostfxr candidate");
    match Host::start(config(&alternate, &bridge)) {
        Err(Error::RuntimeAlreadyLoaded { .. }) => (),
        _ => panic!("switching installations must fail before trying to load another hostfxr"),
    }

    let restarted = Host::start(config(root, &bridge)).expect("restart after shutdown");
    assert_ne!(restarted.session_id().expect("fresh session"), first);
    drop(restarted);
    Host::start(config(root, &bridge))
        .expect("drop also releases the managed session")
        .stop()
        .expect("final shutdown");
}
