use super::*;
use crate::app::{LoadedPlugin, Phase};
use aftercast::{Host, PluginConfig};
use camino::Utf8Path;

pub(in crate::app) async fn fixture(root: &Utf8Path) -> (Host, AppState) {
    let host = Host::start(super::super::tests::config(root))
        .await
        .expect("isolated worker");
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.revision.session = 1;
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = host
        .create_plugin(
            PluginConfig::builder()
                .path(path.clone())
                .type_name("Aftercast.PluginFixture.CharacterImportPlugin")
                .build(),
        )
        .await
        .expect("synthetic plugin");
    let status = host.initialize_plugin(id).await.expect("initialize");
    ui.plugins.push(LoadedPlugin {
        path: path.into_string(),
        id,
        status,
        info: host.plugin_status(id).await.expect("status"),
    });
    (host, ui)
}
pub(in crate::app) fn choose(ui: &mut AppState, mode: &str, name: &str) {
    assert!(ui.edit_import_character(ui.text_view(), "mode", mode));
    if mode == "manual" {
        assert!(ui.edit_import_character(ui.text_view(), "name", name));
    }
}
async fn execute(host: &Host, ui: &mut AppState, path: &Utf8Path) -> super::super::FullRequest {
    let selection = ui.select_full_import().expect("picker selection");
    assert!(ui.select_full_import().is_none());
    let request = ui
        .admit_full_path(selection.token, path.to_owned())
        .expect("admission");
    assert!(!ui.edit_import_character(ui.text_view(), "mode", "legacy"));
    assert_eq!(
        ui.active_import_character(),
        Some(request.viewpoint.clone())
    );
    // Force an independent future draft to prove execution/display borrow only the admitted request.
    let draft = ui.import_text.character.clone();
    ui.import_text.character = Draft {
        mode: Mode::Manual,
        name: "Future identity".into(),
    };
    assert_eq!(
        ui.active_import_character(),
        Some(request.viewpoint.clone())
    );
    let result = super::super::start_selected(
        host,
        path,
        None,
        request.options(),
        request.sources.as_ref(),
        request.viewpoint.as_ref(),
    )
    .await;
    let result = match result {
        Ok(job) => job.wait().await,
        Err(error) => Err(error),
    };
    ui.screen = "analysis";
    assert!(ui.receive_full_import(request.token, result, "complete", "canceled"));
    assert!(ui.active_import_character().is_none());
    ui.import_text.character = draft;
    request
}
#[test]
fn character_drafts_validate_without_rewriting_and_reject_old_render_events() {
    let mut ui = AppState::new("/synthetic".into());
    let old = ui.text_view();
    choose(&mut ui, "manual", "한글");
    assert!(!ui.edit_import_character(old, "name", "stale"));
    assert_eq!(
        ui.import_text
            .viewpoint()
            .expect("valid")
            .expect("explicit")
            .name(),
        "한글"
    );
    assert_eq!(
        description(ui.import_text.viewpoint().expect("valid").as_ref()),
        "한글 → YOU"
    );
    for name in ["", "😀", "😀A", " Target "] {
        assert!(ui.edit_import_character(ui.text_view(), "name", name));
        assert_eq!(
            ui.import_text
                .viewpoint()
                .expect("valid")
                .expect("explicit")
                .name(),
            name
        );
    }
    for name in [
        "bad\nname".into(),
        "x".repeat(1025),
        "한".repeat(342),
        "\u{ffff}".into(),
    ] {
        assert!(ui.edit_import_character(ui.text_view(), "name", &name));
        ui.import_text
            .viewpoint()
            .expect_err("retained invalid draft");
        assert_eq!(ui.import_text.character.name, name);
    }
    choose(&mut ui, "current", "");
    assert_eq!(
        ui.import_text.viewpoint().expect("current"),
        Some(ImportCharacter::current())
    );
    choose(&mut ui, "legacy", "");
    assert_eq!(ui.import_text.viewpoint().expect("legacy"), None);
    assert_eq!(
        AppState::new("/synthetic".into())
            .import_text
            .viewpoint()
            .expect("fresh app default"),
        None
    );
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and synthetic Host; run mise run test:gui-state"]
async fn character_ui_contract_full_requests_bind_viewpoint_and_keep_failure_drafts() {
    let temporary = tempfile::tempdir().expect("temporary synthetic source");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    let (host, mut ui) = fixture(root).await;
    let path = root.join("source.log");
    std::fs::write(&path, "1|Source|Target|100\n2|Target|Source|40\n").expect("synthetic source");
    choose(&mut ui, "manual", "Target");
    let stale = ui.text_view();
    let selected = ui.select_full_import().expect("selection");
    assert!(!ui.edit_import_character(stale, "name", "Source"));
    assert!(ui.abandon_full_selection(selected.token));
    assert!(ui.admit_full_path(selected.token, path.clone()).is_none());
    assert_eq!(
        ui.import_text
            .viewpoint()
            .expect("draft")
            .expect("manual")
            .name(),
        "Target"
    );
    let completed = execute(&host, &mut ui, &path).await;
    assert!(ui.error.is_none());
    assert!(
        host.encounters(0, 16)
            .await
            .expect("graph")
            .items
            .iter()
            .any(|e| e.character == "Target" && e.damage == 40 && e.combatants == 2)
    );
    assert!(!ui.receive_full_import(
        completed.token,
        Err(aftercast::Error::Closed),
        "late",
        "late"
    ));
    assert!(!ui.edit_import_character(stale, "name", "late"));
    execute(&host, &mut ui, &root.join("missing.log")).await;
    assert!(ui.error.is_some());
    assert_eq!(ui.import_text.character.name, "Target");
    choose(&mut ui, "manual", "bad\nname");
    assert!(!ui.can_full_import());
    assert!(ui.select_full_import().is_none());
    choose(&mut ui, "current", "");
    execute(&host, &mut ui, &path).await;
    assert!(
        host.encounters(0, 16)
            .await
            .expect("restored Source")
            .items
            .iter()
            .any(|e| e.character == "Source" && e.damage == 100)
    );
    ui.revision.session += 1;
    ui.import_text.reset_session();
    ui.phase = Phase::Stopping;
    assert!(!ui.receive_full_import(
        completed.token,
        Err(aftercast::Error::Closed),
        "stale",
        "stale"
    ));
    assert_eq!(
        ui.import_text.viewpoint().expect("kept choice"),
        Some(ImportCharacter::current())
    );
    host.shutdown().await.expect("shutdown");
    let (host, mut ui) = fixture(root).await;
    assert_eq!(ui.import_text.viewpoint().expect("new app defaults"), None);
    execute(&host, &mut ui, &path).await;
    assert!(ui.error.is_none());
    host.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and synthetic Host; run mise run test:gui-state"]
async fn character_companion_ui_contract_reuses_selected_sources_and_codec_snapshot() {
    use crate::app::companions::Kind;
    let temporary = tempfile::tempdir().expect("synthetic companion sources");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    let (host, mut ui) = fixture(root).await;
    let main = root.join("main.log");
    std::fs::write(&main, "1|Source|Target|100\n3|Target|Source|40\n").expect("main");
    std::fs::create_dir(root.join("CompanionLogs")).expect("side folder");
    std::fs::write(
        root.join("CompanionLogs/main.Alpha.log"),
        "2|Target|Source|30\n",
    )
    .expect("side");
    ui.import_path = main.to_string();
    let token = ui.begin_sources(Kind::Catalog).expect("explicit catalog");
    assert!(ui.receive_catalog(token, host.companion_candidates(&main).await));
    assert!(
        ui.edit_source(ui.source_view(), "Alpha", "selected", "")
            .expect("selected companion")
    );
    assert!(
        ui.edit_import_text(ui.text_view(), "encoding", "utf8")
            .expect("codec draft")
    );
    choose(&mut ui, "manual", "Target");
    let completed = execute(&host, &mut ui, &main).await;
    assert!(completed.sources.is_some());
    assert_eq!(
        completed.options(),
        Some(aftercast::ImportTextOptions::default())
    );
    assert!(ui.error.is_none());
    assert!(
        host.encounters(0, 16)
            .await
            .expect("graph")
            .items
            .iter()
            .any(|e| e.character == "Target" && e.damage == 70 && e.combatants == 2)
    );
    host.shutdown().await.expect("shutdown");
}
