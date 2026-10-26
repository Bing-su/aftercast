mod common;

use aftercast::{
    CombatRevision, CombatSnapshot, CombatSnapshotStatus, EncounterStatus, Error, Host,
    LogTimeKind, PluginConfig,
};
use camino::Utf8Path;
use common::{Deployment, deployment};

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn completed_snapshots_are_owned_versioned_and_recover_from_partial_imports(
    deployment: Deployment,
) {
    let directory = deployment.directory();
    let config = || deployment.config();
    let mut host = Host::start(config())
        .await
        .expect("start combat snapshot host");
    assert!(
        host.live_combat()
            .await
            .expect("initial live query")
            .is_none()
    );
    assert_eq!(
        host.encounters(0, 16).await.expect("initial history").total,
        0
    );
    assert!(matches!(
        host.encounters(0, 0).await,
        Err(Error::InvalidCombatRequest(_))
    ));
    assert!(matches!(
        host.encounters(0, 257).await,
        Err(Error::InvalidCombatRequest(_))
    ));
    assert!(matches!(
        host.encounters(u32::MAX, 1).await,
        Err(Error::InvalidCombatRequest(_))
    ));
    host.end_combat()
        .await
        .expect("idle completion before initialization");
    assert!(
        host.combat_snapshot(None)
            .await
            .expect("no initial result")
            .is_none()
    );
    let plugin_path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(&plugin_path)
                .type_name("Aftercast.PluginFixture.ImportPlugin")
                .build(),
        )
        .await
        .expect("construct import parser fixture");
    host.initialize_plugin(plugin)
        .await
        .expect("initialize import parser fixture");
    assert!(
        host.combat_snapshot(None)
            .await
            .expect("no ended combat yet")
            .is_none()
    );
    let path = directory.join("snapshot 한글.log");
    std::fs::write(&path, "0|100\n2|300\n").expect("write fixture records");
    host.import_log(&path)
        .await
        .expect("complete fixture import");
    let retained = host
        .combat_snapshot(None)
        .await
        .expect("query completed import")
        .expect("completed import result");
    check_completed(&retained);
    let page = host
        .encounters(0, 1)
        .await
        .expect("list imported encounter");
    assert_eq!(page.total, 1);
    let first_id = page.items[0].id;
    assert!(page.items[0].is_import);
    assert_eq!(page.items[0].status, EncounterStatus::Ended);
    let detail = host
        .encounter_detail(first_id)
        .await
        .expect("query encounter")
        .expect("retained encounter");
    assert_eq!(detail.summary.damage, 400);
    assert_eq!(detail.combatants.len(), 2);
    assert!(
        host.live_combat()
            .await
            .expect("idle after import")
            .is_none()
    );
    let attacks = host
        .encounter_attacks(first_id, 0, 256)
        .await
        .expect("attack query")
        .expect("retained attacks");
    let strike = attacks
        .items
        .iter()
        .find(|row| row.combatant == "Source" && row.category == "Damage" && row.attack == "Strike")
        .expect("damage attack");
    assert!(strike.outgoing);
    assert_eq!(strike.damage, 400);
    assert_eq!(strike.hits, 2);
    assert_eq!(
        host.encounter_attacks(first_id, 1, 1)
            .await
            .expect("attack page")
            .expect("encounter")
            .items
            .len(),
        1
    );
    assert!(
        host.encounters(1, 1)
            .await
            .expect("empty history page")
            .items
            .is_empty()
    );
    assert_eq!(retained.superseded_versions, 0);
    assert!(
        host.combat_snapshot(Some(retained.revision))
            .await
            .expect("unchanged poll")
            .is_none()
    );

    // Each completed import publishes an encounter version and a final import-status version.
    host.import_log(&path).await.expect("repeat fixture import");
    host.import_log(&path)
        .await
        .expect("repeat without polling");
    let coalesced = host
        .combat_snapshot(Some(retained.revision))
        .await
        .expect("query latest coalesced result")
        .expect("new result");
    check_completed(&coalesced);
    assert_eq!(
        coalesced.revision.sequence(),
        retained.revision.sequence() + 4
    );
    assert_eq!(coalesced.superseded_versions, 3);
    let recovered = check_partial_and_recovery(&mut host, &path, coalesced.revision).await;
    let history = host.encounters(0, 256).await.expect("all imports retained");
    assert_eq!(history.total, 5);
    assert_eq!(history.items[0].id, first_id);
    assert_eq!(history.items[3].status, EncounterStatus::PartialImport);
    assert_eq!(
        host.encounter_detail(first_id)
            .await
            .expect("old import detail")
            .expect("old import")
            .summary
            .damage,
        400
    );
    check_completed(&retained);
    host.deinitialize_plugin(plugin)
        .await
        .expect("deinitialize import parser");
    host.shutdown().await.expect("stop snapshot host");

    // Returned strings, actors and timestamps remain owned after later imports and CLR session teardown.
    check_completed(&retained);
    check_completed(&recovered);
    let mut restarted = Host::start(config())
        .await
        .expect("restart managed session");
    assert!(matches!(
        restarted.encounter_detail(first_id).await,
        Err(Error::InvalidCombatRequest(_))
    ));
    assert!(matches!(
        restarted.encounter_attacks(first_id, 0, 1).await,
        Err(Error::InvalidCombatRequest(_))
    ));
    assert_eq!(detail.summary.damage, 400);
    assert_eq!(strike.damage, 400);
    assert!(matches!(
        restarted.combat_snapshot(Some(retained.revision)).await,
        Err(Error::InvalidSnapshotRequest(_))
    ));
    assert!(
        restarted
            .combat_snapshot(None)
            .await
            .expect("restart has no old result")
            .is_none()
    );
    check_live_completion(&mut restarted, directory, &plugin_path).await;
    restarted
        .shutdown()
        .await
        .expect("stop restarted snapshot host");
    check_completed(&retained);
}

async fn check_live_completion(host: &mut Host, directory: &Utf8Path, plugin_path: &Utf8Path) {
    use std::io::Write;
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(plugin_path)
                .type_name("Aftercast.PluginFixture.LiveCombatPlugin")
                .build(),
        )
        .await
        .expect("construct live file parser fixture");
    host.initialize_plugin(plugin)
        .await
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
            host.session_id()
                .await
                .expect("service live parser callbacks");
            assert!(
                started.elapsed() < std::time::Duration::from_secs(5),
                "live records were not admitted"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(matches!(
            host.import_log(&path).await,
            Err(Error::Managed { command: 8, .. })
        ));
        let live = wait_for_live_totals(host, damage, started).await;
        assert_eq!(live.summary.status, EncounterStatus::Active);
        assert!(!live.summary.is_import);
        assert_eq!(live.combatants.len(), 2);
        host.end_combat()
            .await
            .expect("finish live combat without another log record");
        let result = host
            .combat_snapshot(None)
            .await
            .expect("read live result")
            .expect("ended live result");
        assert!(!result.is_import);
        assert_eq!(result.status, CombatSnapshotStatus::Ended);
        assert_eq!(result.damage, damage);
        assert!(
            host.live_combat()
                .await
                .expect("idle after combat")
                .is_none()
        );
        let ended = host
            .encounter_detail(live.summary.id)
            .await
            .expect("same live ID after end")
            .expect("ended combat");
        assert_eq!(ended.summary.status, EncounterStatus::Ended);
        assert_eq!(ended.summary.damage, damage);
        assert_eq!(live.summary.status, EncounterStatus::Active);
        host.end_combat()
            .await
            .expect("idle completion is harmless");
        assert!(
            host.combat_snapshot(Some(result.revision))
                .await
                .expect("unchanged live poll")
                .is_none()
        );
    }
    assert_eq!(host.diagnostics().await.expect("live diagnostics").total, 0);
    host.deinitialize_plugin(plugin)
        .await
        .expect("deinitialize live parser");
}

async fn wait_for_live_totals(
    host: &mut Host,
    damage: i64,
    started: std::time::Instant,
) -> aftercast::EncounterDetail {
    loop {
        host.session_id().await.expect("pump live query");
        if let Some(live) = host
            .live_combat()
            .await
            .expect("read active combat")
            .filter(|live| live.summary.damage == damage)
        {
            return live;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "live combat totals did not update"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
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

async fn check_partial_and_recovery(
    host: &mut Host,
    path: &Utf8Path,
    after: CombatRevision,
) -> CombatSnapshot {
    let mut malformed = b"0|25\n".to_vec();
    malformed.push(0xff);
    std::fs::write(path, malformed).expect("write valid record followed by invalid UTF-8");
    assert!(matches!(
        host.import_log(path).await,
        Err(Error::Managed { command: 8, .. })
    ));
    let partial = host
        .combat_snapshot(Some(after))
        .await
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
    host.import_log(path)
        .await
        .expect("recover after partial import");
    let recovered = host
        .combat_snapshot(Some(partial.revision))
        .await
        .expect("query recovered import")
        .expect("recovered result");
    check_completed(&recovered);
    assert_eq!(
        recovered.revision.sequence(),
        partial.revision.sequence() + 2
    );
    recovered
}
