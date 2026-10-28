use super::*;
#[test]
fn interchange_paths_and_draft_changes_preserve_explicit_intent() {
    let root = camino::Utf8Path::new("/tmp");
    path(root, "").expect_err("empty path");
    path(root, "x\ny").expect_err("control chars");
    assert_eq!(
        path(root, "file.act").expect("relative path"),
        root.join("file.act")
    );
    let mut ui = AppState::new(root.to_owned());
    let view = ui.interchange_view();
    assert!(ui.interchange_edit(view, "output", "new.act".into()));
    assert_eq!(ui.interchange.draft.output, "new.act");
    assert!(!ui.review_act_export(view).expect("idle guard"));
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
        let temporary = tempfile::tempdir().expect("temporary ACT UI");
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
                .application_executable(Utf8PathBuf::try_from(executable).expect("UTF8 exe"))
                .build(),
        )
        .await
        .expect("Host");
        let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll",
        );
        let id = host
            .create_plugin(
                PluginConfig::builder()
                    .path(&path)
                    .type_name("Aftercast.PluginFixture.ImportPlugin")
                    .build(),
            )
            .await
            .expect("parser");
        let status = host.initialize_plugin(id).await.expect("initialize");
        let mut ui = AppState::new(root.clone());
        ui.phase = Phase::Ready;
        ui.revision.session = 1;
        ui.plugins.push(super::super::LoadedPlugin {
            path: path.to_string(),
            id,
            status,
            info: host.plugin_status(id).await.expect("status"),
        });
        let source = root.join("main.log");
        std::fs::write(&source, "1|10\n3|20\n").expect("synthetic log");
        host.import_log(source).await.expect("seed");
        ui.encounters = host.encounters(0, 16).await.expect("rows").items;
        ui.selected = Some(ui.encounters.first().expect("row").id);
        Self {
            _temporary: temporary,
            root,
            host,
            ui,
        }
    }
    fn export(&mut self, name: &str, format: ActFormat, include_logs: bool) -> (Token, Request) {
        let view = self.ui.interchange_view();
        assert!(
            self.ui
                .interchange_edit(view, "output", self.root.join(name).to_string())
        );
        assert!(
            self.ui.interchange_edit(
                view,
                "format",
                if format == ActFormat::Xml {
                    "xml"
                } else {
                    "gzip"
                }
                .into()
            )
        );
        assert!(
            self.ui
                .interchange_edit(view, "logs", include_logs.to_string())
        );
        assert!(self.ui.review_act_export(view).expect("review"));
        let confirm = self.ui.interchange_view();
        let work = self.ui.confirm_act_export(confirm).expect("confirmation");
        assert!(self.ui.confirm_act_export(confirm).is_none());
        work
    }
    async fn run(&mut self, work: (Token, Request)) -> bool {
        let result = perform(&self.host.handle(), &work.1).await;
        self.ui.receive_interchange(work.0, result, "done")
    }
    async fn import(&mut self, path: Utf8PathBuf) -> bool {
        let view = self.ui.interchange_view();
        assert!(self.ui.interchange_edit(view, "input", path.to_string()));
        let work = self
            .ui
            .begin_act_import(view)
            .expect("valid path")
            .expect("import admission");
        assert!(
            self.ui
                .begin_act_import(view)
                .expect("duplicate guard")
                .is_none()
        );
        self.run(work).await
    }
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:gui-state"]
async fn interchange_ui_contract_roundtrips_log_policy_and_preserves_existing_destinations() {
    let mut f = Fixture::new().await;
    for (format, extension) in [(ActFormat::Xml, "xml"), (ActFormat::Gzip, "act")] {
        for include in [true, false] {
            let name = format!("output-{include}.{extension}");
            let work = f.export(&name, format, include);
            assert!(f.run(work).await);
            assert!(f.ui.interchange.error.is_none());
            let path = f.root.join(&name);
            let original = std::fs::read(&path).expect("exported bytes");
            let selected = f.ui.selected;
            let before = f.host.encounters(0, 32).await.expect("before import").total;
            assert!(f.import(path.clone()).await);
            assert!(f.ui.selected.is_none());
            assert!(!f.ui.detail_loading);
            let imported = f.host.encounters(0, 32).await.expect("graph");
            assert_eq!(imported.total, before + 1);
            let last = imported.items.last().expect("added row");
            assert_eq!(last.damage, 30);
            assert_eq!(
                f.host
                    .encounter_logs(last.id, 0, 16)
                    .await
                    .expect("logs")
                    .expect("encounter")
                    .page
                    .total,
                if include { 2 } else { 0 }
            );
            f.ui.encounters = imported.items;
            f.ui.selected = selected;
            let work = f.export(&name, format, include);
            assert!(f.run(work).await);
            assert!(f.ui.interchange.error.is_some());
            assert_eq!(std::fs::read(&path).expect("existing bytes"), original);
            assert_eq!(f.ui.interchange.draft.output, path.to_string());
        }
    }
    let work = f.export("raced.xml", ActFormat::Xml, true);
    let raced = f.root.join("raced.xml");
    std::fs::write(&raced, b"synthetic competing writer").expect("race after confirmation");
    assert!(f.run(work).await);
    assert!(f.ui.interchange.error.is_some());
    assert_eq!(
        std::fs::read(&raced).expect("race preserved"),
        b"synthetic competing writer"
    );
    f.host.shutdown().await.expect("shutdown");
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:gui-state"]
async fn interchange_ui_contract_empty_invalid_input_and_save_failure_keep_graph_and_drafts() {
    let mut f = Fixture::new().await;
    let count = f.host.encounters(0, 32).await.expect("count").total;
    f.ui.selected = None;
    let view = f.ui.interchange_view();
    f.ui.interchange_edit(view, "output", "none.xml".into());
    assert_eq!(
        f.ui.review_act_export(view).expect_err("no target"),
        "act-select-encounter"
    );
    assert!(!f.root.join("none.xml").exists());
    for (name, bytes) in [
        ("empty.xml", b"<Encounters/>".as_slice()),
        ("bad.xml", b"<Encounters><bad>".as_slice()),
        ("bad.act", &[0x1f, 0x8b, 0x08, 0x00][..]),
        ("zero.xml", &[][..]),
    ] {
        let path = f.root.join(name);
        std::fs::write(&path, bytes).expect("synthetic input");
        assert!(f.import(path.clone()).await);
        assert_eq!(f.ui.interchange.error.is_none(), name == "empty.xml");
        assert_eq!(f.ui.interchange.draft.input, path.to_string());
        assert_eq!(
            f.host.encounters(0, 32).await.expect("atomic graph").total,
            count
        );
    }
    f.ui.encounters = f.host.encounters(0, 32).await.expect("targets").items;
    f.ui.selected = Some(f.ui.encounters.first().expect("target").id);
    let work = f.export("missing/output.xml", ActFormat::Xml, false);
    assert!(f.run(work).await);
    assert!(f.ui.interchange.error.is_some());
    assert!(!f.root.join("missing/output.xml").exists());
    assert!(!f.ui.interchange.draft.options.include_logs);
    let destination = f.root.join("directory-destination");
    std::fs::create_dir(&destination).expect("synthetic directory");
    let work = f.export("directory-destination", ActFormat::Xml, false);
    assert!(f.run(work).await);
    assert!(f.ui.interchange.error.is_some());
    assert!(destination.is_dir());
    assert!(
        !std::fs::read_dir(&f.root)
            .expect("directory")
            .any(|entry| entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp"))
    );
    f.host.shutdown().await.expect("shutdown");
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:gui-state"]
async fn interchange_ui_contract_dialog_cancel_stale_confirmation_and_admission_snapshot() {
    let mut f = Fixture::new().await;
    let path = f.root.join("cancel.xml");
    let view = f.ui.interchange_view();
    f.ui.interchange_edit(view, "output", path.to_string());
    f.ui.interchange_edit(view, "format", "xml".into());
    let picker =
        f.ui.begin_interchange_picker(view, Direction::Export)
            .expect("picker");
    assert!(f.ui.receive_interchange_picker(&picker, None));
    assert!(!path.exists());
    let picker =
        f.ui.begin_interchange_picker(f.ui.interchange_view(), Direction::Import)
            .expect("picker");
    f.ui.interchange.leave();
    assert!(
        !f.ui
            .receive_interchange_picker(&picker, Some(f.root.join("late.xml")))
    );
    assert!(f.ui.interchange.draft.input.is_empty());
    let view = f.ui.interchange_view();
    assert!(f.ui.review_act_export(view).expect("review"));
    let old = f.ui.interchange_view();
    assert!(f.ui.cancel_act_confirmation(old));
    assert!(f.ui.confirm_act_export(old).is_none());
    assert!(!path.exists());
    f.ui.review_act_export(old).expect("review A");
    let stale = f.ui.interchange_view();
    f.ui.revision.selection += 1;
    assert!(f.ui.confirm_act_export(stale).is_none());
    let view = f.ui.interchange_view();
    f.ui.review_act_export(view).expect("current review");
    let current = f.ui.interchange_view();
    assert!(f.ui.confirm_act_export(stale).is_none());
    let work = f.ui.confirm_act_export(current).expect("current admission");
    assert!(!f.ui.interchange_edit(current, "logs", "false".into()));
    f.ui.interchange.draft.options.include_logs = false; // Simulate an unrelated newer draft after admission.
    f.ui.interchange.leave();
    assert!(f.run(work).await);
    let xml = std::fs::read_to_string(path).expect("accepted write");
    assert_eq!(xml.matches("<LogLine ").count(), 2);
    f.host.shutdown().await.expect("shutdown");
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:gui-state"]
async fn interchange_ui_contract_shutdown_drains_accepted_export_and_rejects_late_response() {
    use std::{future::Future, task::Poll};
    let mut f = Fixture::new().await;
    let work = f.export("drained.xml", ActFormat::Xml, false);
    let handle = f.host.handle();
    let mut operation = Box::pin(perform(&handle, &work.1));
    let pending =
        std::future::poll_fn(|cx| Poll::Ready(operation.as_mut().poll(cx).is_pending())).await;
    if pending {
        handle.diagnostics().await.expect("canonicalize barrier");
        let pending =
            std::future::poll_fn(|cx| Poll::Ready(operation.as_mut().poll(cx).is_pending())).await;
        if pending {
            handle
                .diagnostics()
                .await
                .expect("archive admission barrier");
        }
    }
    drop(operation);
    f.ui.revision.session += 1;
    f.ui.interchange.stop();
    f.ui.phase = Phase::Stopping;
    f.host.shutdown().await.expect("drain accepted archive I/O");
    assert!(f.root.join("drained.xml").exists());
    assert!(!f.ui.receive_interchange(work.0, Ok(1), "late"));
    assert!(f.ui.phase == Phase::Stopping);
}

#[test]
fn interchange_render_and_picker_reservations_are_invalidated_before_native_open() {
    let temporary = tempfile::tempdir().expect("UI state");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.revision.session = 1;
    let old = ui.interchange_view();
    ui.interchange.leave();
    assert!(ui.begin_act_import(old).expect("stale event").is_none());
    let picker = ui
        .begin_interchange_picker(ui.interchange_view(), Direction::Import)
        .expect("picker");
    assert!(ui.current_interchange_picker(&picker));
    ui.interchange.leave();
    assert!(!ui.current_interchange_picker(&picker));
    assert!(!ui.receive_interchange_picker(&picker, Some(root.join("late.act"))));
    assert!(ui.interchange.draft.input.is_empty());
    let picker = ui
        .begin_interchange_picker(ui.interchange_view(), Direction::Export)
        .expect("picker");
    ui.exit_requested = true;
    assert!(!ui.current_interchange_picker(&picker));
    assert!(!ui.receive_interchange_picker(&picker, None));
    ui.exit_requested = false;
    let picker = ui
        .begin_interchange_picker(ui.interchange_view(), Direction::Export)
        .expect("picker");
    ui.revision.selection += 1;
    assert!(!ui.current_interchange_picker(&picker));
    assert!(!ui.receive_interchange_picker(&picker, Some(root.join("different-target.act"))));
    assert!(ui.interchange.draft.output.is_empty());
    let picker = ui
        .begin_interchange_picker(ui.interchange_view(), Direction::Import)
        .expect("picker");
    ui.revision.session += 1;
    ui.interchange.stop();
    ui.phase = Phase::Stopping;
    assert!(!ui.current_interchange_picker(&picker));
    assert!(!ui.receive_interchange_picker(&picker, None));
    assert!(ui.phase == Phase::Stopping);
}

#[test]
fn archive_log_default_is_separate_from_request_draft() {
    let mut ui = AppState::new("/isolated".into());
    ui.interchange.apply_defaults_once(true);
    assert!(ui.interchange.draft.options.include_logs);
    let view = ui.interchange_view();
    assert!(ui.interchange_edit(view, "logs", "false".into()));
    ui.interchange.apply_defaults_once(true);
    assert!(!ui.interchange.draft.options.include_logs);
    ui.interchange.stop();
    ui.interchange.apply_defaults_once(true);
    assert!(!ui.interchange.draft.options.include_logs);

    let mut early = AppState::new("/isolated".into());
    let view = early.interchange_view();
    assert!(early.interchange_edit(view, "output", "draft.xml".into()));
    early.interchange.apply_defaults_once(false);
    assert!(early.interchange.draft.options.include_logs);
}
