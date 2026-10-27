use super::*;
#[test]
fn draft_preserves_explicit_intent() {
    let mut ui = AppState::new("/tmp".into());
    let old = ui.report_token();
    assert!(ui.report_edit(old, "player", "한글{name}".into()));
    assert!(!ui.report_edit(old, "player", "stale".into()));
    assert_eq!(ui.reports.draft.player_format, "한글{name}");
    assert!(
        ui.report_begin(ui.report_token(), true)
            .expect("idle")
            .is_none()
    );
}

use aftercast::{Host, HostConfig, PluginConfig, extract_bridge};
use camino::Utf8Path;
struct Fixture {
    _temporary: tempfile::TempDir,
    root: Utf8PathBuf,
    host: Host,
    ui: AppState,
}
impl Fixture {
    async fn new() -> Self {
        let temporary = tempfile::tempdir().expect("temporary reports");
        let root = Utf8Path::from_path(temporary.path())
            .expect("UTF8")
            .to_owned();
        let bridge = root.join("bridge");
        extract_bridge(&bridge).expect("payload");
        let executable = std::env::current_exe()
            .expect("tests")
            .parent()
            .and_then(std::path::Path::parent)
            .expect("target")
            .join("examples")
            .join(format!("test-application{}", std::env::consts::EXE_SUFFIX));
        let host = Host::start(
            HostConfig::builder()
                .dotnet_root(std::env::var("DOTNET_ROOT").expect("runtime"))
                .bridge_directory(bridge)
                .data_directory(root.join("state"))
                .application_executable(Utf8PathBuf::try_from(executable).expect("UTF8"))
                .build(),
        )
        .await
        .expect("Host");
        let plugin=host.create_plugin(PluginConfig::builder().path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll")).type_name("Aftercast.PluginFixture.TextReportPlugin").build()).await.expect("plugin");
        host.initialize_plugin(plugin).await.expect("initialize");
        let source = root.join("synthetic.log");
        std::fs::write(&source, "0|100\n1|200\n").expect("source");
        host.import_log(source).await.expect("import");
        let mut ui = AppState::new(root.clone());
        ui.phase = Phase::Ready;
        ui.revision.session = 1;
        ui.encounters = host.encounters(0, 16).await.expect("encounters").items;
        ui.selected = Some(ui.encounters[0].id);
        Self {
            _temporary: temporary,
            root,
            host,
            ui,
        }
    }
    async fn load(&mut self) {
        let (token, request) = self
            .ui
            .report_begin(self.ui.report_token(), false)
            .expect("admit")
            .expect("request");
        let result = perform(&self.host.handle(), &request).await;
        assert!(self.ui.report_receive(token, result, ""));
        assert!(self.ui.report_preset(self.ui.report_token(), Some(0)));
    }
    fn render(&mut self) -> (Token, Request) {
        self.ui
            .report_begin(self.ui.report_token(), true)
            .expect("render")
            .expect("request")
    }
    async fn preview(&mut self) {
        let (token, request) = self.render();
        let result = perform(&self.host.handle(), &request).await;
        assert!(self.ui.report_receive(token, result, ""));
        assert!(self.ui.report_preview().is_some());
    }
    fn save(&mut self, name: &str) -> (Token, Request) {
        assert!(self.ui.report_edit(
            self.ui.report_token(),
            "path",
            self.root.join(name).to_string()
        ));
        assert!(
            self.ui
                .report_review_save(self.ui.report_token())
                .expect("review")
        );
        self.ui
            .report_confirm_save(self.ui.report_token())
            .expect("save")
    }
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn reports_ui_contract_snapshot_save_failure_cancel_and_stale_events() {
    let mut f = Fixture::new().await;
    f.load().await;
    f.preview().await;
    let generated = f.ui.report_preview().expect("preview").report.text.clone();
    assert_eq!(generated, "Target:한글日本😀Source:한글日本😀\n");
    let (token, request) = f.save("report.txt");
    assert!(f.ui.report_confirm_save(token).is_none());
    assert!(!f.ui.report_edit(token, "player", "raced".into()));
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, "saved"));
    assert_eq!(
        std::fs::read(f.root.join("report.txt")).expect("saved"),
        generated.as_bytes()
    );
    assert!(
        f.ui.report_preview().is_some(),
        "snapshot remains available after save"
    );
    let (token, request) = f.save("report.txt");
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, "saved"));
    assert!(f.ui.reports.error.is_some());
    assert!(f.ui.report_preview().is_some());
    assert_eq!(
        std::fs::read_to_string(f.root.join("report.txt")).expect("original"),
        generated
    );
    let (token, request) = f.save("missing/report.txt");
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.error.is_some());
    let (token, request) = f.save("canceled.txt");
    assert!(f.ui.report_cancel(token));
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert!(!f.root.join("canceled.txt").exists());
    f.preview().await;
    assert!(f.ui.report_edit(
        f.ui.report_token(),
        "path",
        f.root.join("old.txt").to_string()
    ));
    f.ui.report_review_save(f.ui.report_token())
        .expect("old review");
    let old = f.ui.report_token();
    assert!(f.ui.report_cancel_review(old));
    f.ui.report_review_save(f.ui.report_token())
        .expect("new review");
    assert!(f.ui.report_confirm_save(old).is_none());
    f.ui.report_cancel_review(f.ui.report_token());
    let (token, request) = f.render();
    assert!(f.ui.report_begin(token, true).expect("duplicate").is_none());
    f.ui.revision.selection += 1;
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert!(f.ui.phase == Phase::Ready);
    assert!(f.ui.report_preview().is_none());
    let (token, request) = f.render();
    f.ui.reports.leave();
    f.ui.screen = "history";
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    f.ui.screen = "analysis";
    f.preview().await;
    let (completed_token, completed_request) = f.save("completed.txt");
    let completed = perform(&f.host.handle(), &completed_request).await;
    f.ui.reports.stop();
    f.ui.revision.session += 1;
    f.ui.phase = Phase::Idle;
    assert!(!f.ui.report_receive(completed_token, completed, "late save"));
    assert_eq!(
        std::fs::read_to_string(f.root.join("completed.txt")).expect("accepted save completed"),
        generated
    );
    f.ui.phase = Phase::Ready;
    f.load().await;
    f.preview().await;
    let (token, request) = f.save("stopped.txt");
    f.ui.reports.stop();
    f.ui.revision.session += 1;
    f.ui.phase = Phase::Idle;
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert!(!f.root.join("stopped.txt").exists());
    f.host.shutdown().await.expect("shutdown");
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn reports_ui_contract_drafts_failures_catalog_refresh_and_session_retirement() {
    let mut f = Fixture::new().await;
    f.load().await;
    let token = f.ui.report_token();
    assert!(f.ui.report_edit(token, "player", "{report-fail}".into()));
    assert!(!f.ui.report_preset(token, Some(0)));
    let (token, request) = f.render();
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.error.is_some());
    assert_eq!(f.ui.reports.draft.player_format, "{report-fail}");
    assert!(f.ui.report_edit(f.ui.report_token(), "player", "{report-mutate}".into()));
    let (token, request) = f.render();
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.error.is_some());
    let (token, request) =
        f.ui.report_begin(f.ui.report_token(), false)
            .expect("reload")
            .expect("request");
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert_eq!(
        f.ui.reports.draft.player_format, "{report-mutate}",
        "reload preserves explicit draft"
    );
    assert!(f.ui.report_edit(f.ui.report_token(), "player", "한글{report-added}".into()));
    f.preview().await;
    let token = f.ui.report_token();
    assert!(f.ui.report_edit(token, "player", "new draft".into()));
    assert!(f.ui.report_preview().is_none());
    assert!(!f.ui.report_review_save(token).expect("stale review"));
    f.ui.report_review_save(f.ui.report_token())
        .expect_err("generate first");
    let (token, request) = f.render();
    assert!(f.ui.report_cancel(token));
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert_eq!(f.ui.reports.draft.player_format, "new draft");
    assert!(f.ui.report_edit(f.ui.report_token(), "player", "".into()));
    f.preview().await;
    assert_eq!(
        f.ui.report_preview()
            .expect("empty template report")
            .report
            .text,
        "\n"
    );
    assert!(f.ui.report_edit(f.ui.report_token(), "player", "{report-remove}".into()));
    let (token, request) = f.render();
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.error.is_some());
    assert!(f.ui.report_edit(f.ui.report_token(), "player", "{report-name}".into()));
    let (token, request) = f.render();
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert_eq!(f.ui.reports.error.as_deref(), Some("report-unavailable"));
    assert!(f.ui.report_preview().is_none());

    let (token, request) =
        f.ui.report_begin(f.ui.report_token(), false)
            .expect("load")
            .expect("request");
    f.ui.revision.session += 1;
    f.ui.reports.stop();
    f.ui.phase = Phase::Idle;
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.catalog.is_none());
    f.host.shutdown().await.expect("shutdown");
}
