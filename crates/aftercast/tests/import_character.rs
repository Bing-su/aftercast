mod common;
use aftercast::{
    CompanionSource, EncounterStatus, Error, HistoryQuery, Host, ImportCharacter, ImportRange,
    ImportTextOptions, PluginConfig, SourceImportOptions,
};
use camino::Utf8Path;
use common::{Deployment, deployment};
use std::time::Duration;

async fn setup(deployment: &Deployment, short: bool) -> Host {
    let host = Host::start(deployment.config())
        .await
        .expect("isolated worker");
    let parser = host.create_plugin(PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll"))
        .type_name(if short { "Aftercast.PluginFixture.ShortCharacterImportPlugin" } else { "Aftercast.PluginFixture.CharacterImportPlugin" }).build()).await.expect("synthetic parser");
    host.initialize_plugin(parser).await.expect("initialize");
    host
}
fn options(name: &str) -> SourceImportOptions {
    SourceImportOptions {
        viewpoint: Some(ImportCharacter::new(name).expect("choice")),
        ..Default::default()
    }
}
const COMBAT: &str = "1|Source|Target|100\n2|Target|Source|40\n";

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn file_range_companion_and_text_choices_reach_the_isolated_parser(deployment: Deployment) {
    let main = deployment.directory().join("main.log");
    std::fs::write(&main, COMBAT).expect("synthetic source");
    let host = setup(&deployment, false).await;
    assert_eq!(
        host.import_log(&main).await.expect("legacy current").damage,
        100
    );
    for (name, damage) in [
        ("Target", 40),
        ("", 100),
        ("Unknown actor", 0),
        (" Target ", 0),
    ] {
        let mut selected = options(name);
        let job = host
            .start_import_sources(&main, selected.clone())
            .await
            .expect("Begin snapshot");
        selected.viewpoint = Some(ImportCharacter::new("Unrelated").expect("caller change"));
        assert_eq!(job.wait().await.expect("owned viewpoint").damage, damage);
    }
    let epoch = 621_355_968_000_000_000;
    let mut selected = options("Target");
    selected.range =
        Some(ImportRange::new(epoch + 20_000_000, epoch + 20_000_000).expect("inclusive range"));
    selected.text = Some(ImportTextOptions::default());
    assert_eq!(
        host.import_sources(&main, selected)
            .await
            .expect("range codec viewpoint")
            .damage,
        40
    );
    let sides = deployment.directory().join("CompanionLogs");
    std::fs::create_dir(&sides).expect("side directory");
    std::fs::write(sides.join("main.Alpha.log"), "2|Target|Source|30\n").expect("companion");
    let mut selected = options("Target");
    selected.companions.push(CompanionSource {
        name: "Alpha".into(),
        text: None,
    });
    assert_eq!(
        host.import_sources(&main, selected)
            .await
            .expect("shared companion identity")
            .damage,
        70
    );
    for name in ["", "Al", "한글", "😀", "😀A", "手動 이름"] {
        let choice = ImportCharacter::new(name).expect("Unicode selection");
        assert_eq!(
            host.start_clipboard_import_with_viewpoint("1|SELF|Target|11", choice)
                .await
                .expect("supplied text Begin")
                .wait()
                .await
                .expect("self alias")
                .damage,
            11
        );
    }
    assert_eq!(
        host.import_log(&main)
            .await
            .expect("restored live Source")
            .damage,
        100
    );
    host.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn omitted_and_explicit_current_remain_distinct_across_worker_restart(
    deployment: Deployment,
) {
    let main = deployment.directory().join("main.log");
    std::fs::write(&main, "1|Al|YOU|7\n2|YOU|Al|11\n").expect("short current fixture");
    for _ in 0..2 {
        let host = setup(&deployment, true).await;
        assert_eq!(
            host.import_log(&main)
                .await
                .expect("legacy exact Al")
                .damage,
            7
        );
        assert_eq!(
            host.import_sources(&main, options(""))
                .await
                .expect("explicit current fallback YOU")
                .damage,
            11
        );
        assert_eq!(
            host.import_clipboard_text("1|Al|YOU|7\n2|YOU|Al|11")
                .await
                .expect("legacy supplied text")
                .damage,
            7
        );
        assert_eq!(
            host.start_clipboard_import_with_viewpoint(
                "1|Al|YOU|7\n2|YOU|Al|11",
                ImportCharacter::current()
            )
            .await
            .expect("explicit text")
            .wait()
            .await
            .expect("fallback")
            .damage,
            11
        );
        let page = host
            .encounters(0, 100)
            .await
            .expect("exact identity metadata");
        assert!(page.items.iter().any(|e| e.character == "Al"));
        assert!(page.items.iter().any(|e| e.character == "YOU"));
        host.save_history()
            .await
            .expect("persist selected ally graph");
        host.shutdown().await.expect("shutdown");
        let reader = Host::start(deployment.config())
            .await
            .expect("history reader worker");
        let page = reader
            .query_history(&HistoryQuery {
                limit: 16,
                ..Default::default()
            })
            .await
            .expect("saved selection metadata");
        assert!(page.items.iter().any(|e| e.character == "Al"));
        assert!(page.items.iter().any(|e| e.character == "YOU"));
        reader.shutdown().await.expect("reader shutdown");
    }
}

#[rstest::rstest]
#[case(false)]
#[case(true)]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn failure_cancel_and_retry_restore_current_identity(
    deployment: Deployment,
    #[case] supplied_text: bool,
) {
    let main = deployment.directory().join("main.log");
    let host = setup(&deployment, false).await;
    std::fs::write(&main, "1|Target|Source|40\nbad timestamp\n").expect("partial input");
    let failed = if supplied_text {
        host.start_clipboard_import_with_viewpoint(
            "1|Target|Source|40\nbad timestamp\n",
            ImportCharacter::new("Target").expect("choice"),
        )
        .await
        .expect("text admission")
        .wait()
        .await
    } else {
        host.import_sources(&main, options("Target")).await
    };
    assert!(matches!(failed, Err(Error::Managed { .. })));
    let graph = host.encounters(0, 10).await.expect("partial graph");
    assert!(
        graph
            .items
            .iter()
            .any(|e| e.status == EncounterStatus::PartialImport && e.damage == 40)
    );
    std::fs::write(
        &main,
        "1|Target|Source|40|gate\n".to_owned() + &COMBAT.repeat(5000),
    )
    .expect("cancel source");
    let job = if supplied_text {
        host.start_clipboard_import_with_viewpoint(
            std::fs::read_to_string(&main).expect("owned synthetic text"),
            ImportCharacter::new("Target").expect("choice"),
        )
        .await
    } else {
        host.start_import_sources(&main, options("Target")).await
    }
    .expect("cancel Begin");
    tokio::time::timeout(Duration::from_secs(5), async {
        while !deployment.state.join("character.ready").exists() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("callback gate");
    job.cancel();
    std::fs::write(deployment.state.join("character.release"), "release").expect("release");
    assert!(matches!(job.wait().await, Err(Error::Cancelled)));
    std::fs::write(&main, COMBAT).expect("retry input");
    assert_eq!(
        host.import_log(&main)
            .await
            .expect("legacy after cancel")
            .damage,
        100
    );
    assert_eq!(
        host.import_sources(&main, options("Target"))
            .await
            .expect("new selection after cancel")
            .damage,
        40
    );
    host.shutdown().await.expect("shutdown");
}
