use super::*;
use aftercast::{Host, HostConfig, extract_bridge};
use camino::{Utf8Path, Utf8PathBuf};

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:gui-state"]
async fn speech_rule_ui_contract_drafts_save_and_stale_sessions() {
    let temp = tempfile::tempdir().expect("isolated data");
    let root = Utf8Path::from_path(temp.path()).expect("UTF8");
    let bridge = root.join("bridge");
    extract_bridge(&bridge).expect("bridge");
    let executable = std::env::current_exe().expect("test executable");
    let application = executable
        .parent()
        .and_then(std::path::Path::parent)
        .expect("target")
        .join("examples")
        .join(format!("test-application{}", std::env::consts::EXE_SUFFIX));
    let host = Host::start(
        HostConfig::builder()
            .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
            .bridge_directory(bridge)
            .data_directory(root.join("state"))
            .application_executable(Utf8PathBuf::try_from(application).expect("UTF8"))
            .build(),
    )
    .await
    .expect("host");
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.screen = "settings";
    ui.revision.session = 1;
    let (token, _, _) = ui.begin_speech_rules(false).expect("load");
    assert!(ui.begin_speech_rules(false).is_none());
    let loaded = host.speech_rules().await;
    assert!(ui.receive_speech_rules(token, false, loaded, String::new()));
    let token = ui.speech_rule_token();
    assert!(
        ui.edit_speech_rule(token, true, |editor| editor.draft.push(SpeechRule {
            pattern: "[".into(),
            replacement: "x".into(),
            active: true
        }))
    );
    assert!(!ui.edit_speech_rule(token, false, |_| {}), "old row token");
    let draft = ui.speech_rules.draft.clone();
    let (token, revision, submitted) = ui.begin_speech_rules(true).expect("save");
    let failure = host
        .save_speech_rules(revision.expect("revision"), &submitted)
        .await;
    failure.as_ref().expect_err("invalid regex");
    assert!(ui.receive_speech_rules(token, true, failure, "saved".into()));
    assert_eq!(ui.speech_rules.draft, draft, "failure preserves draft");
    assert!(ui.speech_rules.error.is_some());
    let token = ui.speech_rule_token();
    ui.speech_rules.confirm_discard = true;
    assert!(ui.discard_speech_rules(token, false));
    assert_eq!(ui.speech_rules.draft, draft);
    let token = ui.speech_rule_token();
    assert!(
        ui.edit_speech_rule(token, false, |editor| editor.draft[0].pattern =
            "cat".into())
    );
    let (token, revision, submitted) = ui.begin_speech_rules(true).expect("valid save");
    let saved = host
        .save_speech_rules(revision.expect("revision"), &submitted)
        .await;
    ui.screen = "analysis";
    assert!(ui.receive_speech_rules(token, true, saved, "saved".into()));
    assert!(!ui.speech_rules.dirty);
    assert_eq!(ui.message, "saved");
    verify_reorder_stale_and_stop(&host, &mut ui).await;
    host.shutdown().await.expect("stop");
}

async fn verify_reorder_stale_and_stop(host: &Host, ui: &mut AppState) {
    ui.screen = "settings";
    let token = ui.speech_rule_token();
    assert!(
        ui.edit_speech_rule(token, true, |editor| editor.draft.push(SpeechRule {
            pattern: "dog".into(),
            replacement: "hound".into(),
            active: true
        }))
    );
    let token = ui.speech_rule_token();
    assert!(ui.edit_speech_rule(token, true, |editor| editor.draft.swap(0, 1)));
    assert_eq!(ui.speech_rules.draft[0].pattern, "dog");
    let revision = ui
        .speech_rules
        .snapshot
        .as_ref()
        .expect("saved snapshot")
        .revision;
    host.save_speech_rules(
        revision,
        &[SpeechRule {
            pattern: "external".into(),
            replacement: "changed".into(),
            active: true,
        }],
    )
    .await
    .expect("concurrent edit");
    let (token, revision, submitted) = ui.begin_speech_rules(true).expect("stale save");
    let result = host
        .save_speech_rules(revision.expect("revision"), &submitted)
        .await;
    result.as_ref().expect_err("stale");
    assert!(ui.receive_speech_rules(token, true, result, "saved".into()));
    assert_eq!(
        ui.speech_rules.draft, submitted,
        "stale failure preserves reordered draft"
    );
    let token = ui.speech_rule_token();
    ui.speech_rules.confirm_discard = true;
    assert!(ui.discard_speech_rules(token, true));
    let (token, _, _) = ui.begin_speech_rules(false).expect("load before stop");
    let reply = host.speech_rules().await;
    ui.revision.session += 1;
    ui.speech_rules = Default::default();
    ui.phase = Phase::Stopping;
    assert!(!ui.receive_speech_rules(token, false, reply, String::new()));
}
