use super::*;
use aftercast::{Host, HostConfig, extract_bridge};
use camino::Utf8Path;

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:gui-state"]
async fn correction_ui_contract_preserves_drafts_and_rejects_stale_sessions() {
    let temporary = tempfile::tempdir().expect("fixture");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF-8");
    let bridge = root.join("bridge");
    extract_bridge(&bridge).expect("bridge");
    let executable = std::env::current_exe().expect("test executable");
    let app = executable
        .parent()
        .and_then(std::path::Path::parent)
        .expect("target")
        .join("examples")
        .join(format!("test-application{}", std::env::consts::EXE_SUFFIX));
    let config = HostConfig::builder()
        .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
        .bridge_directory(bridge)
        .data_directory(root.join("state"))
        .application_executable(camino::Utf8PathBuf::try_from(app).expect("UTF-8"))
        .build();
    let host = Host::start(config.clone()).await.expect("host");
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.screen = "settings";
    ui.revision.session = 7;
    let (load, _, _, _) = ui.begin_corrections(false).expect("load");
    assert!(ui.begin_corrections(false).is_none(), "duplicate load");
    let snapshot = host.corrections().await.expect("snapshot");
    ui.screen = "analysis";
    assert!(
        ui.receive_corrections(load, Ok(snapshot)),
        "navigation must not lose admitted response"
    );
    let token = ui.correction_token();
    assert!(!ui.correction_edit(token, true, |_| {}), "late hidden edit");
    ui.screen = "settings";
    assert!(
        ui.correction_edit(token, true, |editor| editor.renames.push(
            RenameDefinition {
                before: "".into(),
                after: "Hero".into(),
                active: true
            }
        ))
    );
    assert!(!ui.correction_edit(token, true, |_| {}), "old row event");
    assert!(
        ui.begin_corrections(false).is_none(),
        "load cannot erase draft"
    );
    let (save, revision, renames, redirects) = ui.begin_corrections(true).expect("save");
    assert!(ui.begin_corrections(true).is_none(), "duplicate save");
    let error = host
        .save_corrections(revision.expect("revision"), &renames, &redirects)
        .await
        .expect_err("empty name");
    assert!(ui.receive_corrections(save, Err(error)));
    assert!(
        ui.corrections.dirty && ui.corrections.renames[0].after == "Hero",
        "failed save preserves draft"
    );
    let token = ui.correction_token();
    assert!(
        ui.correction_edit(token, false, |editor| editor.renames[0].before =
            "Source".into())
    );
    assert!(ui.request_correction_discard(token));
    assert!(ui.confirm_correction_discard(token, false));
    assert!(ui.corrections.dirty, "cancel discard");
    assert!(ui.request_correction_discard(token));
    assert!(ui.confirm_correction_discard(token, true));
    assert!(!ui.corrections.dirty && ui.corrections.renames.is_empty());
    let token = ui.correction_token();
    assert!(
        ui.correction_edit(token, true, |editor| editor.renames.push(
            RenameDefinition {
                before: "Source".into(),
                after: "Hero".into(),
                active: true
            }
        ))
    );
    let (save, revision, renames, redirects) = ui.begin_corrections(true).expect("save");
    let result = host
        .save_corrections(revision.expect("revision"), &renames, &redirects)
        .await;
    assert!(ui.receive_corrections(save, result));
    assert!(!ui.corrections.dirty && ui.corrections.renames.len() == 1);
    stale_and_late_save(&host, &mut ui).await;
    host.shutdown().await.expect("stop");
    let host = Host::start(config).await.expect("restart");
    assert!(
        host.corrections()
            .await
            .expect("persisted deletion")
            .renames
            .is_empty()
    );
    host.shutdown().await.expect("stop");
}

async fn stale_and_late_save(host: &Host, ui: &mut AppState) {
    let token = ui.correction_token();
    assert!(
        ui.correction_edit(token, false, |editor| editor.renames[0].after =
            "Local".into())
    );
    let old_revision = ui.corrections.snapshot.as_ref().expect("snapshot").revision;
    host.save_corrections(
        old_revision,
        &[RenameDefinition {
            before: "Source".into(),
            after: "Remote".into(),
            active: true,
        }],
        &[],
    )
    .await
    .expect("remote edit");
    let (save, revision, renames, redirects) = ui.begin_corrections(true).expect("stale save");
    let result = host
        .save_corrections(revision.expect("revision"), &renames, &redirects)
        .await;
    assert!(ui.receive_corrections(save, result));
    assert!(
        ui.corrections.dirty && ui.corrections.renames[0].after == "Local",
        "stale failure retains draft"
    );
    let token = ui.correction_token();
    assert!(ui.request_correction_discard(token));
    assert!(ui.confirm_correction_discard(token, true));
    let (load, _, _, _) = ui.begin_corrections(false).expect("load");
    let late = host.corrections().await.expect("snapshot");
    ui.revision.session += 1;
    ui.corrections = EditorState::default();
    ui.phase = Phase::Ready;
    assert!(
        !ui.receive_corrections(load, Ok(late)),
        "old session response"
    );

    let (load, _, _, _) = ui.begin_corrections(false).expect("reload");
    assert!(ui.receive_corrections(load, host.corrections().await));
    assert_eq!(ui.corrections.renames[0].after, "Remote");
    let token = ui.correction_token();
    ui.screen = "analysis";
    assert!(!ui.correction_edit(token, true, |_| {}));
    ui.screen = "settings";
    assert!(ui.correction_edit(token, true, |editor| {
        editor.renames.remove(0);
    }));
    let (save, revision, renames, redirects) = ui.begin_corrections(true).expect("delete");
    let saved = host
        .save_corrections(revision.expect("revision"), &renames, &redirects)
        .await;
    ui.revision.session += 1;
    ui.corrections = EditorState::default();
    ui.phase = Phase::Ready;
    assert!(
        !ui.receive_corrections(save, saved),
        "old save acknowledgement"
    );
}
