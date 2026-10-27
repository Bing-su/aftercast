use super::*;

#[test]
fn source_scope_and_capture_budget_require_explicit_bounded_input() {
    let root = camino::Utf8Path::new("/tmp");
    Draft::default().query(root).expect_err("explicit scope");
    let mut draft = Draft {
        path: "synthetic.log".into(),
        ..Default::default()
    };
    assert_eq!(
        draft.query(root).expect("scope").paths,
        vec![root.join("synthetic.log")]
    );
    assert_eq!(
        draft.limit().expect("default budget"),
        4 * 1024 * 1024 * 1024
    );
    for value in ["0", "4097", "bad"] {
        draft.limit = value.into();
        draft.limit().expect_err("invalid budget");
    }
}

use aftercast::{HistoryQuery, Host, HostConfig, PluginConfig, extract_bridge};
use camino::Utf8Path;
struct Fixture {
    _temporary: tempfile::TempDir,
    root: Utf8PathBuf,
    host: Host,
    ui: AppState,
    selection: Selection,
}
impl Fixture {
    async fn new() -> Self {
        let temporary = tempfile::tempdir().expect("synthetic GUI sources");
        let root = Utf8Path::from_path(temporary.path())
            .expect("UTF8")
            .to_owned();
        let bridge = root.join("bridge");
        extract_bridge(&bridge).expect("payload");
        let executable = std::env::current_exe()
            .expect("test executable")
            .parent()
            .and_then(std::path::Path::parent)
            .expect("target")
            .join("examples")
            .join(format!("test-application{}", std::env::consts::EXE_SUFFIX));
        let host = Host::start(
            HostConfig::builder()
                .dotnet_root(std::env::var("DOTNET_ROOT").expect("trusted runtime"))
                .bridge_directory(bridge)
                .data_directory(root.join("state"))
                .application_executable(Utf8PathBuf::try_from(executable).expect("UTF8 executable"))
                .build(),
        )
        .await
        .expect("isolated worker");
        let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll",
        );
        let id = host
            .create_plugin(
                PluginConfig::builder()
                    .path(path.clone())
                    .type_name("Aftercast.PluginFixture.ImportPlugin")
                    .build(),
            )
            .await
            .expect("fixture plugin");
        let status = host.initialize_plugin(id).await.expect("initialize");
        let mut ui = AppState::new(root.clone());
        ui.phase = Phase::Ready;
        ui.screen = "history";
        ui.revision.session = 1;
        ui.plugins.push(super::super::LoadedPlugin {
            path: path.to_string(),
            id,
            status,
            info: host.plugin_status(id).await.expect("status"),
        });
        std::fs::write(root.join("main.log"), "1|10\n3|20\n").expect("synthetic source");
        host.import_log(root.join("main.log")).await.expect("seed");
        host.save_history().await.expect("history");
        let page = host
            .query_history(&HistoryQuery::default())
            .await
            .expect("page");
        let selection = Selection {
            revision: page.revision,
            entry: page.items.first().expect("checkpoint").clone(),
        };
        Self {
            _temporary: temporary,
            root,
            host,
            ui,
            selection,
        }
    }
    fn begin(&mut self, action: SourceAction) -> Work {
        let view = self.ui.history_source_view();
        let handle = self.host.handle();
        let work = self
            .ui
            .begin_source_work(view, action.clone(), handle.clone())
            .expect("validated")
            .expect("admitted");
        assert!(
            self.ui
                .begin_source_work(view, action, handle)
                .expect("duplicate guard")
                .is_none()
        );
        work
    }
    async fn drive(&mut self, action: SourceAction) -> Option<aftercast::ImportReport> {
        let work = self.begin(action);
        let token = work.token;
        let action = work.action.clone();
        let selection = work.selection.clone();
        let mut output = perform(work, |_| {}).await;
        let mut report = None;
        if let Some(job) = output.job.take() {
            match job.wait().await {
                Ok(value) => report = Some(value),
                Err(error) => output.error = Some(error.to_string()),
            }
            if let Err(error) = output.owned.release().await {
                output.error = Some(join_error(output.error, error));
            }
        }
        assert!(
            self.ui
                .finish_source_work(token, &action, selection, output)
                .is_none()
        );
        report
    }
    async fn discover(&mut self) {
        self.drive(SourceAction::History(Box::new(self.selection.clone())))
            .await;
        self.ui.history_sources.draft.path = "main.log".into();
        self.drive(SourceAction::Discover).await;
        assert!(self.ui.history_sources.snapshot.is_some());
        assert!(self.ui.history_sources.path.is_none());
        self.drive(SourceAction::Path(self.root.join("main.log")))
            .await;
    }
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:gui-state"]
async fn history_sources_ui_contract_captures_explicit_snapshots_and_releases_on_selection_change()
{
    let mut f = Fixture::new().await;
    let sides = f.root.join("CompanionLogs");
    std::fs::create_dir(&sides).expect("sides");
    std::fs::write(sides.join("main.Alpha.log"), "1|100\n3|200\n9|999\n").expect("side");
    f.discover().await;
    f.ui.history_sources.choices.push(Choice {
        name: "Alpha".into(),
        text: Default::default(),
    });
    f.ui.history_sources
        .draft
        .text
        .change("encoding", "utf8")
        .expect("codec");
    f.drive(SourceAction::Capture).await;
    assert!(f.ui.history_sources.ready);
    assert_eq!(f.ui.history_sources.files.len(), 2);
    let old_view = f.ui.history_source_view();
    let admitted = f.ui.history_sources.admitted.clone().expect("snapshot");
    assert_eq!(admitted.options.companions.len(), 1);
    assert_eq!(admitted.options.maximum_bytes, 4 * 1024 * 1024 * 1024);
    assert_eq!(
        admitted.range.end_ticks() - admitted.range.start_ticks(),
        20_000_000
    );
    std::fs::write(f.root.join("main.log"), "9|9999\n").expect("replacement");
    std::fs::remove_file(sides.join("main.Alpha.log")).expect("remove synthetic side");
    f.ui.selected = Some(
        f.host
            .encounters(0, 1)
            .await
            .expect("old graph")
            .items
            .first()
            .expect("encounter")
            .id,
    );
    f.ui.detail_loading = true;
    let report = f.drive(SourceAction::Replay).await.expect("replay");
    assert!(f.ui.selected.is_none());
    assert!(!f.ui.detail_loading);
    assert_eq!(
        (
            report.records_dispatched,
            report.damage,
            report.companion_heads_excluded
        ),
        (4, 330, 1)
    );
    assert!(!f.ui.history_sources.ready);
    assert!(!f.ui.history_sources.holds_resources());
    f.drive(SourceAction::Discover).await;
    f.drive(SourceAction::Path(f.root.join("main.log"))).await;
    f.drive(SourceAction::Capture).await;
    assert!(
        f.ui.begin_source_work(old_view, SourceAction::Replay, f.host.handle())
            .expect("stale replay guard")
            .is_none()
    );
    assert!(
        f.ui.begin_source_work(old_view, SourceAction::Release, f.host.handle())
            .expect("stale release guard")
            .is_none()
    );
    assert!(f.ui.history_sources.ready);
    assert!(f.ui.leave_source_view());
    f.drive(SourceAction::Release).await;
    assert!(!f.ui.history_sources.holds_resources());
    assert_eq!(f.ui.history_sources.draft.path, "main.log");
    f.drive(SourceAction::History(Box::new(f.selection.clone())))
        .await;
    f.drive(SourceAction::Discover).await;
    f.drive(SourceAction::History(Box::new(f.selection.clone())))
        .await;
    assert!(!f.ui.history_sources.holds_resources());
    f.host.shutdown().await.expect("shutdown");
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:gui-state"]
async fn history_sources_ui_contract_cancel_stale_response_and_stop_keep_cleanup_owned() {
    let mut f = Fixture::new().await;
    f.drive(SourceAction::History(Box::new(f.selection.clone())))
        .await;
    f.ui.history_sources.draft.path = "main.log".into();
    let work = f.begin(SourceAction::Discover);
    let token = work.token;
    let action = work.action.clone();
    let selected = work.selection.clone();
    assert!(!f.ui.leave_source_view());
    let output = perform(work, |_| {}).await;
    assert!(output.canceled);
    assert!(!output.owned.active());
    f.ui.finish_source_work(token, &action, selected, output);
    assert!(f.ui.history_sources.snapshot.is_none());
    assert!(f.ui.phase == Phase::Ready);
    let sides = f.root.join("CompanionLogs");
    std::fs::create_dir(&sides).expect("sides");
    std::fs::write(sides.join("main.Alpha.log"), "2|100\n").expect("synthetic side");
    f.discover().await;
    let mut codec = import_text::Draft::default();
    codec.change("encoding", "cp949").expect("side codec");
    f.ui.history_sources.choices.push(Choice {
        name: "Alpha".into(),
        text: codec.clone(),
    });
    let work = f.begin(SourceAction::Capture);
    let token = work.token;
    let action = work.action.clone();
    let selected = work.selection.clone();
    f.ui.cancel_source_work();
    let output = perform(work, |_| {}).await;
    assert!(output.canceled);
    assert!(!output.owned.active());
    f.ui.finish_source_work(token, &action, selected, output);
    assert_eq!(
        f.ui.history_sources
            .choices
            .first()
            .expect("cancel preserves companion draft")
            .text,
        codec
    );
    assert!(!f.ui.history_sources.ready);
    assert!(!f.ui.history_sources.holds_resources());
    f.discover().await;
    let work = f.begin(SourceAction::Capture);
    let token = work.token;
    let action = work.action.clone();
    let selected = work.selection.clone();
    let output = perform(work, |_| {}).await;
    assert!(output.owned.active());
    f.ui.revision.session += 1;
    f.ui.history_sources.stop();
    let mut old =
        f.ui.finish_source_work(token, &action, selected, output)
            .expect("old session resources returned for cleanup");
    old.release().await.expect("explicit late cleanup");
    assert!(!f.ui.history_sources.holds_resources());
    assert!(!f.ui.history_sources.ready);
    f.ui.phase = Phase::Importing;
    f.ui.cancel_import = false;
    assert!(!f.ui.leave_source_view());
    assert!(
        !f.ui.cancel_import,
        "leaving unrelated import must not cancel it"
    );
    f.host.shutdown().await.expect("drain source I/O");
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:gui-state"]
async fn history_sources_ui_contract_failures_keep_drafts_and_allow_release_retry() {
    let mut f = Fixture::new().await;
    f.discover().await;
    let sides = f.root.join("CompanionLogs");
    std::fs::create_dir(&sides).expect("sides");
    let side = sides.join("main.Alpha.log");
    std::fs::write(&side, "2|100\n").expect("side");
    f.drive(SourceAction::Path(f.root.join("main.log"))).await;
    f.ui.history_sources.choices.push(Choice {
        name: "Alpha".into(),
        text: Default::default(),
    });
    std::fs::remove_file(side).expect("delete synthetic side");
    f.drive(SourceAction::Capture).await;
    assert!(f.ui.history_sources.error.is_some());
    assert!(!f.ui.history_sources.ready);
    assert_eq!(f.ui.history_sources.choices.len(), 1);
    assert_eq!(f.ui.history_sources.draft.path, "main.log");
    f.drive(SourceAction::Release).await;
    assert!(!f.ui.history_sources.holds_resources());
    f.drive(SourceAction::Discover).await;
    f.drive(SourceAction::Path(f.root.join("main.log"))).await;
    f.drive(SourceAction::Capture).await;
    assert!(f.ui.history_sources.ready);
    let index = f.root.join("state/history/index.xml");
    let original = std::fs::read(&index).expect("index");
    let mut changed = original.clone();
    changed.push(b' ');
    std::fs::write(&index, changed).expect("synthetic revision change");
    f.drive(SourceAction::Replay).await;
    assert!(f.ui.history_sources.error.is_some());
    assert!(
        f.ui.history_sources.ready,
        "failed preflight must preserve ready image"
    );
    std::fs::write(index, original).expect("restore fixture index");
    f.drive(SourceAction::Release).await;
    assert!(!f.ui.history_sources.holds_resources());
    assert_eq!(f.ui.history_sources.draft.path, "main.log");
    f.host.shutdown().await.expect("cleanup");
}
