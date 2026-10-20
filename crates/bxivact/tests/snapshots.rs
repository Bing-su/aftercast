use bxivact::{
    CombatRevision, CombatSnapshot, CombatSnapshotStatus, Error, Host, HostConfig, LogTimeKind,
    PluginConfig, extract_bridge,
};
use camino::Utf8Path;

#[test]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
fn completed_snapshots_are_owned_versioned_and_recover_from_partial_imports() {
    let root = std::env::var("DOTNET_ROOT").expect("set a UTF-8 DOTNET_ROOT");
    let temporary = tempfile::tempdir().expect("create combat snapshot fixture");
    let directory = Utf8Path::from_path(temporary.path()).expect("UTF-8 test directory");
    let bridge = directory.join("bridge");
    extract_bridge(&bridge).expect("extract managed component");
    let config = || {
        HostConfig::builder()
            .dotnet_root(&root)
            .bridge_directory(&bridge)
            .data_directory(directory.join("state"))
            .build()
    };
    let mut host = Host::start(config()).expect("start combat snapshot host");
    host.end_combat()
        .expect("idle completion before initialization");
    assert!(
        host.combat_snapshot(None)
            .expect("no initial result")
            .is_none()
    );
    let plugin_path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Bxivact.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(&plugin_path)
                .type_name("Bxivact.PluginFixture.ImportPlugin")
                .build(),
        )
        .expect("construct import parser fixture");
    host.initialize_plugin(plugin)
        .expect("initialize import parser fixture");
    assert!(
        host.combat_snapshot(None)
            .expect("no ended combat yet")
            .is_none()
    );
    let path = directory.join("snapshot 한글.log");
    std::fs::write(&path, "0|100\n2|300\n").expect("write fixture records");
    host.import_log(&path).expect("complete fixture import");
    let retained = host
        .combat_snapshot(None)
        .expect("query completed import")
        .expect("completed import result");
    check_completed(&retained);
    assert_eq!(retained.superseded_versions, 0);
    assert!(
        host.combat_snapshot(Some(retained.revision))
            .expect("unchanged poll")
            .is_none()
    );

    // Each completed import publishes an encounter version and a final import-status version.
    host.import_log(&path).expect("repeat fixture import");
    host.import_log(&path).expect("repeat without polling");
    let coalesced = host
        .combat_snapshot(Some(retained.revision))
        .expect("query latest coalesced result")
        .expect("new result");
    check_completed(&coalesced);
    assert_eq!(
        coalesced.revision.sequence(),
        retained.revision.sequence() + 4
    );
    assert_eq!(coalesced.superseded_versions, 3);
    let recovered = check_partial_and_recovery(&mut host, &path, coalesced.revision);
    check_completed(&retained);
    host.deinitialize_plugin(plugin)
        .expect("deinitialize import parser");
    host.stop().expect("stop snapshot host");

    // Returned strings, actors and timestamps remain owned after later imports and CLR session teardown.
    check_completed(&retained);
    check_completed(&recovered);
    let mut restarted = Host::start(config()).expect("restart managed session");
    assert!(matches!(
        restarted.combat_snapshot(Some(retained.revision)),
        Err(Error::InvalidSnapshotRequest(_))
    ));
    assert!(
        restarted
            .combat_snapshot(None)
            .expect("restart has no old result")
            .is_none()
    );
    check_live_completion(&mut restarted, directory, &plugin_path);
    restarted.stop().expect("stop restarted snapshot host");
    check_completed(&retained);
}

fn check_live_completion(host: &mut Host, directory: &Utf8Path, plugin_path: &Utf8Path) {
    use std::io::Write;
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(plugin_path)
                .type_name("Bxivact.PluginFixture.LiveCombatPlugin")
                .build(),
        )
        .expect("construct live file parser fixture");
    host.initialize_plugin(plugin)
        .expect("initialize live parser");
    let path = directory.join("state/live.log");
    let marker = directory.join("state/live.accepted");
    for (lines, last, damage, replace) in [
        ("0|100\n2|300\n", "2|300", 400, false),
        ("4|50\n", "4|50", 50, false),
        ("\u{feff}6|75\n", "6|75", 75, true),
    ] {
        if replace {
            // A rotated file already contains its first record; reopening at the tail would lose this encounter.
            std::fs::rename(&path, path.with_extension("previous.log"))
                .expect("rotate the followed file");
            std::fs::write(&path, lines).expect("create replacement with its first live record");
        } else {
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("open followed file")
                .write_all(lines.as_bytes())
                .expect("append live records");
        }
        // Wait for fixture action admission; observing a raw log batch alone precedes the mutable parsing hooks.
        let started = std::time::Instant::now();
        while !std::fs::read_to_string(&marker).is_ok_and(|text| text == last) {
            host.pump().expect("service live parser callbacks");
            assert!(
                started.elapsed() < std::time::Duration::from_secs(5),
                "live records were not admitted"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(matches!(
            host.import_log(&path),
            Err(Error::Managed { command: 8, .. })
        ));
        host.end_combat()
            .expect("finish live combat without another log record");
        let result = host
            .combat_snapshot(None)
            .expect("read live result")
            .expect("ended live result");
        assert!(!result.is_import);
        assert_eq!(result.status, CombatSnapshotStatus::Ended);
        assert_eq!(result.damage, damage);
        host.end_combat().expect("idle completion is harmless");
        assert!(
            host.combat_snapshot(Some(result.revision))
                .expect("unchanged live poll")
                .is_none()
        );
    }
    assert_eq!(host.diagnostics().expect("live diagnostics").total, 0);
    host.deinitialize_plugin(plugin)
        .expect("deinitialize live parser");
}

fn check_completed(snapshot: &CombatSnapshot) {
    assert!(snapshot.is_import);
    assert_eq!(snapshot.status, CombatSnapshotStatus::Ended);
    assert_eq!(snapshot.character, "Source");
    assert_eq!(snapshot.title, "Target");
    assert_eq!(snapshot.damage, 400);
    assert_eq!(snapshot.healed, 0);
    assert_eq!(snapshot.duration_ticks, 20_000_000);
    assert_eq!(snapshot.combatants.len(), 2);
    assert_eq!(snapshot.start_time.kind, LogTimeKind::Utc);
    assert_eq!(snapshot.start_time.ticks, 621_355_968_000_000_000);
    assert_eq!(snapshot.end_time.ticks, 621_355_968_020_000_000);
    let source = snapshot
        .combatants
        .iter()
        .find(|actor| actor.name == "Source")
        .expect("source participant");
    assert!(source.is_ally);
    assert_eq!(source.damage, 400);
    assert_eq!(source.swings, 2);
    assert_eq!(source.hits, 2);
    assert!((source.dps - 200.0).abs() < f64::EPSILON);
    let target = snapshot
        .combatants
        .iter()
        .find(|actor| actor.name == "Target")
        .expect("target participant");
    assert!(!target.is_ally);
    assert_eq!(target.damage_taken, 400);
    assert!(target.dps.is_nan() && target.to_hit.is_nan());
}

fn check_partial_and_recovery(
    host: &mut Host,
    path: &Utf8Path,
    after: CombatRevision,
) -> CombatSnapshot {
    let mut malformed = b"0|25\n".to_vec();
    malformed.push(0xff);
    std::fs::write(path, malformed).expect("write valid record followed by invalid UTF-8");
    assert!(matches!(
        host.import_log(path),
        Err(Error::Managed { command: 8, .. })
    ));
    let partial = host
        .combat_snapshot(Some(after))
        .expect("query retained partial result")
        .expect("partial encounter result");
    assert!(partial.is_import);
    assert_eq!(partial.status, CombatSnapshotStatus::PartialImport);
    assert_eq!(partial.damage, 25);
    assert_eq!(partial.revision.sequence(), after.sequence() + 2);
    let source = partial
        .combatants
        .iter()
        .find(|actor| actor.name == "Source")
        .expect("partial source participant");
    assert!(source.dps.is_infinite() && source.dps.is_sign_positive());
    std::fs::write(path, "0|100\n2|300\n").expect("restore complete fixture");
    host.import_log(path).expect("recover after partial import");
    let recovered = host
        .combat_snapshot(Some(partial.revision))
        .expect("query recovered import")
        .expect("recovered result");
    check_completed(&recovered);
    assert_eq!(
        recovered.revision.sequence(),
        partial.revision.sequence() + 2
    );
    recovered
}
