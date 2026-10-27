use super::*;
use aftercast::{Host, HostConfig, PluginConfig, extract_bridge};

#[test]
fn source_range_drafts_keep_inclusive_dates_and_reject_invalid_input() {
    let root = Utf8Path::new("/isolated");
    let valid = Draft {
        path: " source.log ".into(),
        start: "1970-01-01".into(),
        end: "1970-01-01".into(),
    };
    let request = valid.compile(root).expect("inclusive day");
    assert_eq!(request.path, root.join("source.log"));
    assert_eq!(
        request.range.end_ticks() - request.range.start_ticks(),
        864_000_000_000 - 1
    );
    let exact = Draft {
        start: "1970-01-01T00:00:01.0000001".into(),
        end: "1970-01-01T00:00:01.0000001".into(),
        ..valid.clone()
    };
    let request = exact.compile(root).expect("equal timestamps");
    assert_eq!(request.range.start_ticks(), request.range.end_ticks());
    for draft in [
        Draft {
            path: String::new(),
            ..valid.clone()
        },
        Draft {
            path: "bad\0path".into(),
            ..valid.clone()
        },
        Draft {
            start: String::new(),
            ..valid.clone()
        },
        Draft {
            end: "2025-02-29".into(),
            ..valid.clone()
        },
        Draft {
            end: "1970-01-01T00:00:60".into(),
            ..valid.clone()
        },
        Draft {
            start: "1970-01-02".into(),
            ..valid.clone()
        },
    ] {
        let saved = draft.clone();
        draft.compile(root).expect_err("invalid draft");
        assert_eq!(draft, saved);
    }
    let maximum = Draft {
        start: "9999-12-31".into(),
        end: "9999-12-31".into(),
        ..valid
    };
    assert_eq!(
        maximum
            .compile(root)
            .expect("maximum full day")
            .range
            .end_ticks(),
        3_155_378_975_999_999_999
    );
}
async fn fixture(host: &Host, ui: &mut AppState) {
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = host
        .create_plugin(
            PluginConfig::builder()
                .path(path.clone())
                .type_name("Aftercast.PluginFixture.ImportPlugin")
                .build(),
        )
        .await
        .expect("parser");
    let status = host.initialize_plugin(id).await.expect("initialize");
    ui.plugins.push(super::super::LoadedPlugin {
        path: path.into_string(),
        id,
        status,
        info: host.plugin_status(id).await.expect("status"),
    });
}
fn request(ui: &AppState) -> Request {
    ui.compile_reparse().expect("range draft")
}
async fn execute(host: &Host, ui: &mut AppState) -> Token {
    let request = request(ui);
    let view = ui.reparse_view();
    assert!(ui.current_reparse_view(view));
    let token = ui.begin_reparse(request.clone()).expect("admission");
    assert!(
        !ui.current_reparse_view(view),
        "old rendered submit cannot start another import"
    );
    assert!(
        ui.begin_reparse(request.clone()).is_none(),
        "duplicate request"
    );
    let result = match super::super::import_text::start_selected(
        host,
        &request.path,
        Some(request.range),
        request.text,
        request.sources.as_ref(),
    )
    .await
    {
        Ok(job) => job.wait().await,
        Err(error) => Err(error),
    };
    assert!(ui.receive_reparse(token, result, "reparsed", "canceled"));
    token
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and synthetic fixture; run mise run test:gui-state"]
async fn reparse_ui_contract_preserves_drafts_and_session_owned_partial_results() {
    let temporary = tempfile::tempdir().expect("isolated reparse controller");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8 path");
    let bridge = root.join("bridge");
    extract_bridge(&bridge).expect("bridge");
    let executable = std::env::current_exe().expect("test binary");
    let application = executable
        .parent()
        .and_then(std::path::Path::parent)
        .expect("target")
        .join("examples")
        .join(format!("test-application{}", std::env::consts::EXE_SUFFIX));
    let config = || {
        HostConfig::builder()
            .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
            .bridge_directory(bridge.clone())
            .data_directory(root.join("state"))
            .application_executable(
                camino::Utf8PathBuf::try_from(application.clone()).expect("UTF8 executable"),
            )
            .build()
    };
    let host = Host::start(config()).await.expect("host");
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.revision.session = 1;
    ui.reparse.draft = Draft {
        path: "source.log".into(),
        start: "1970-01-01T00:00:01".into(),
        end: "1970-01-01T00:00:02".into(),
    };
    assert!(
        ui.begin_reparse(request(&ui)).is_none(),
        "initialized parser required"
    );
    fixture(&host, &mut ui).await;
    let input = root.join("source.log");
    std::fs::write(&input, "0|100\n1|200\n1|300\n2|400\n3|999\n").expect("source");
    let draft = ui.reparse.draft.clone();
    ui.loading = true;
    assert!(
        ui.begin_reparse(request(&ui)).is_none(),
        "wait for active Analysis read"
    );
    ui.loading = false;
    let generation = ui.log_windows.generation();
    let token = execute(&host, &mut ui).await;
    assert_eq!(ui.log_windows.generation(), generation + 2);
    assert_eq!(ui.message, "reparsed · 3 / 5 · 1");
    assert_eq!(ui.reparse.draft, draft);
    assert!(!ui.receive_reparse(
        token,
        Err(aftercast::Error::Cancelled),
        "duplicate",
        "duplicate"
    ));
    let graph = host.encounters(0, 16).await.expect("graph");
    let id = graph.items.first().expect("range encounter").id;
    assert_eq!(graph.items.first().expect("range encounter").damage, 900);
    ui.selected = Some(id);
    ui.detail_loading = true;
    failures(&host, &mut ui, &input).await;
    encoding_policy(&host, &mut ui, &input).await;
    signal_loop(&host, &ui, &input).await;
    cancel_and_shutdown(host, &mut ui, &input).await;
    let next = Host::start(config()).await.expect("clean restart");
    next.shutdown().await.expect("restart shutdown");
}
async fn failures(host: &Host, ui: &mut AppState, input: &Utf8Path) {
    let saved = ui.reparse.draft.clone();
    ui.reparse.draft.path = "missing.log".into();
    execute(host, ui).await;
    assert!(ui.error.is_some());
    assert_eq!(ui.reparse.draft.path, "missing.log");
    assert!(ui.selected.is_none());
    assert!(!ui.detail_loading);
    ui.reparse.draft = saved.clone();
    std::fs::write(input, [b"1|100\n".as_slice(), &[255, 10]].concat()).expect("malformed source");
    let before = host.encounters(0, 16).await.expect("before failure").total;
    execute(host, ui).await;
    assert!(ui.error.is_some());
    assert_eq!(ui.reparse.draft, saved);
    let graph = host.encounters(0, 16).await.expect("partial graph");
    assert_eq!(graph.total, before + 1);
    assert!(
        graph.items.iter().any(
            |row| row.status == aftercast::EncounterStatus::PartialImport && row.damage == 100
        )
    );
    ui.reparse.draft.start = "1970-01-02".into();
    ui.reparse.draft.end = "1970-01-02".into();
    std::fs::write(input, "1|100\n").expect("empty range source");
    execute(host, ui).await;
    assert_eq!(ui.message, "reparsed · 0 / 1 · 0");
    ui.reparse.draft = saved;
}
async fn cancel_and_shutdown(host: Host, ui: &mut AppState, input: &Utf8Path) {
    std::fs::write(input, "1|1\n".repeat(100_000)).expect("long source");
    let request = request(ui);
    let before = host
        .encounters(0, 100)
        .await
        .expect("admitted graph")
        .items
        .into_iter()
        .map(|row| row.id)
        .collect::<Vec<_>>();
    let token = ui.begin_reparse(request.clone()).expect("cancel request");
    let job = host
        .start_import_range(input, request.range)
        .await
        .expect("job");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while job.progress().records_dispatched == 0 {
        host.diagnostics().await.expect("responsive");
        assert!(std::time::Instant::now() < deadline, "progress deadline");
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    ui.progress = job.progress();
    assert!(ui.progress.bytes_read <= ui.progress.total_bytes);
    ui.screen = "history";
    assert!(ui.cancel_reparse(token));
    assert!(!ui.cancel_reparse(token));
    job.cancel();
    assert!(ui.receive_reparse(token, job.wait().await, "reparsed", "canceled"));
    assert_eq!(ui.message, "canceled");
    let graph = host.encounters(0, 100).await.expect("canceled graph");
    let added = graph
        .items
        .iter()
        .filter(|row| !before.contains(&row.id))
        .collect::<Vec<_>>();
    assert_eq!(added.len(), 1);
    let partial = added.first().expect("this canceled range encounter");
    assert_eq!(partial.status, aftercast::EncounterStatus::PartialImport);
    assert!(partial.damage > 0);
    assert_eq!(ui.screen, "history");
    assert!(ui.phase == Phase::Ready);
    stale_and_shutdown(host, ui, input, token).await;
}

async fn stale_and_shutdown(host: Host, ui: &mut AppState, input: &Utf8Path, token: Token) {
    let request = request(ui);
    // The completed response must not cancel or acknowledge the next request in this session.
    let current = ui.begin_reparse(request.clone()).expect("next request");
    assert!(!ui.cancel_reparse(token));
    assert!(!ui.receive_reparse(token, Err(aftercast::Error::Cancelled), "late", "late"));
    assert!(ui.reparse.active());
    let job = host
        .start_import_range(input, request.range)
        .await
        .expect("shutdown import");
    let generation = ui.log_windows.generation();
    let draft = ui.reparse.draft.clone();
    ui.revision.session += 1;
    ui.reparse.reset_session();
    ui.phase = Phase::Stopping;
    host.shutdown().await.expect("drained shutdown");
    assert!(!ui.receive_reparse(current, job.wait().await, "late", "late"));
    assert!(!ui.cancel_reparse(current));
    assert_eq!(ui.reparse.draft, draft);
    assert_eq!(ui.log_windows.generation(), generation);
    assert!(ui.phase == Phase::Stopping);
}

async fn signal_loop(host: &Host, ui: &AppState, input: &Utf8Path) {
    let dom = VirtualDom::new(|| rsx! {});
    let mut signal = dom.in_scope(ScopeId::ROOT, || {
        let mut state = AppState::new(ui.root.clone());
        state.plugins = ui.plugins.clone();
        state.phase = Phase::Ready;
        state.revision.session = ui.revision.session;
        state.reparse.draft = ui.reparse.draft.clone();
        Signal::new(state)
    });
    std::fs::write(input, "1|1\n".repeat(100_000)).expect("signal source");
    let request = request(&signal.read());
    let token = signal
        .write()
        .begin_reparse(request.clone())
        .expect("signal admission");
    let job = host
        .start_import_range(input, request.range)
        .await
        .expect("signal job");
    let cancel = async move {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        signal.write().screen = "analysis";
        assert!(signal.write().cancel_reparse(token));
    };
    let (result, ()) = tokio::join!(
        super::super::wait_import(signal, job, token.session),
        cancel
    );
    assert!(matches!(result, Err(aftercast::Error::Cancelled)));
    assert!(
        signal
            .write()
            .receive_reparse(token, result, "reparsed", "canceled")
    );
    assert_eq!(signal.read().message, "canceled");
    assert_eq!(signal.read().screen, "analysis");
    let token = signal
        .write()
        .begin_reparse(request.clone())
        .expect("second signal request");
    let job = host
        .start_import_range(input, request.range)
        .await
        .expect("second signal job");
    let replace = async move {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let mut state = signal.write();
        state.revision.session += 1;
        state.reparse.reset_session();
        state.phase = Phase::Stopping;
        state.progress = Default::default();
    };
    let (result, ()) = tokio::join!(
        super::super::wait_import(signal, job, token.session),
        replace
    );
    assert!(matches!(result, Err(aftercast::Error::Cancelled)));
    assert!(
        !signal
            .write()
            .receive_reparse(token, result, "late", "late")
    );
    assert_eq!(signal.read().progress, aftercast::ImportProgress::default());
    assert!(signal.read().phase == Phase::Stopping);
}

async fn encoding_policy(host: &Host, ui: &mut AppState, input: &Utf8Path) {
    assert!(
        ui.edit_import_text(ui.text_view(), "encoding", "cp949")
            .expect("encoding edit")
    );
    let selected = ui.import_text.options();
    std::fs::write(input,b"0|100|\xc7\xd1\xb1\xdb\n1|200|\xc7\xd1\xb1\xdb\n1|300|\xc7\xd1\xb1\xdb\n2|400|\xc7\xd1\xb1\xdb\n3|999\n").expect("Korean range");
    let compiled = request(ui);
    assert_eq!(compiled.text, selected);
    let token = ui.begin_reparse(compiled.clone()).expect("range admission");
    assert!(
        !ui.edit_import_text(ui.text_view(), "encoding", "ascii")
            .expect("busy edit rejected")
    );
    ui.screen = "analysis";
    let result =
        super::super::import_text::start(host, &compiled.path, Some(compiled.range), compiled.text)
            .await
            .expect("range start")
            .wait()
            .await;
    assert!(ui.receive_reparse(token, result, "reparsed", "cancelled"));
    assert_eq!(ui.message, "reparsed · 3 / 5 · 1");
    assert_eq!(ui.screen, "analysis");
    assert_eq!(ui.import_text.options(), selected);
    let graph = host.encounters(0, 256).await.expect("graph");
    let last = graph.items.last().expect("Korean encounter");
    let logs = host
        .encounter_logs(last.id, 0, 16)
        .await
        .expect("logs")
        .expect("scope");
    assert_eq!(
        logs.page.items.first().expect("first source").text,
        "1|200|한글"
    );
    encoded_partial_failure(host, ui, input).await;
    assert!(
        ui.edit_import_text(ui.text_view(), "invalid", "strict")
            .expect("strict reset")
    );
    assert!(
        ui.edit_import_text(ui.text_view(), "encoding", "plugin")
            .expect("inherit reset")
    );
}
async fn encoded_partial_failure(host: &Host, ui: &mut AppState, input: &Utf8Path) {
    assert!(
        ui.edit_import_text(ui.text_view(), "encoding", "utf8")
            .expect("explicit UTF8")
    );
    let selected = ui.import_text.options();
    let draft = ui.reparse.draft.clone();
    std::fs::write(input, b"1|100|valid\n1|200|\xff\n3|0|stop\n").expect("bad range source");
    let before = host.encounters(0, 256).await.expect("before").total;
    execute(host, ui).await;
    assert!(ui.error.is_some());
    assert_eq!(ui.reparse.draft, draft);
    assert_eq!(ui.import_text.options(), selected);
    let graph = host.encounters(before, 1).await.expect("partial");
    assert_eq!(graph.items.first().expect("partial encounter").damage, 100);
    assert!(
        ui.edit_import_text(ui.text_view(), "invalid", "replace")
            .expect("replace edit")
    );
    execute(host, ui).await;
    assert!(ui.error.is_none());
    let graph = host.encounters(before + 1, 1).await.expect("replacement");
    let id = graph.items.first().expect("replaced encounter").id;
    let logs = host
        .encounter_logs(id, 0, 16)
        .await
        .expect("logs")
        .expect("scope");
    assert!(
        logs.page
            .items
            .get(1)
            .expect("replaced source")
            .text
            .contains('\u{fffd}')
    );
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and synthetic Host; run mise run test:gui-state"]
async fn companion_range_ui_contract_uses_admitted_sources_and_keeps_cancelled_drafts() {
    use super::super::companions::Kind;
    let temporary = tempfile::tempdir().expect("private source fixtures");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    let bridge = root.join("bridge");
    extract_bridge(&bridge).expect("payload");
    let application = std::env::current_exe()
        .expect("test path")
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
    .expect("worker");
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.revision.session = 1;
    fixture(&host, &mut ui).await;
    let main = root.join("source.log");
    std::fs::write(
        &main,
        "0|999
1|10
3|999
",
    )
    .expect("main");
    std::fs::create_dir(root.join("CompanionLogs")).expect("sides");
    std::fs::write(
        root.join("CompanionLogs/source.Alpha.log"),
        "1|21
1|22
4|999
",
    )
    .expect("companion");
    ui.import_path = main.to_string();
    let token = ui.begin_sources(Kind::Catalog).expect("query");
    assert!(ui.receive_catalog(token, host.companion_candidates(&main).await));
    assert!(
        ui.edit_source(ui.source_view(), "Alpha", "selected", "")
            .expect("choose")
    );
    assert!(
        ui.edit_source(ui.source_view(), "Alpha", "encoding", "utf8")
            .expect("codec")
    );
    ui.reparse.draft = Draft {
        path: main.to_string(),
        start: "1970-01-01T00:00:01".into(),
        end: "1970-01-01T00:00:01".into(),
    };
    let compiled = request(&ui);
    let token = ui.begin_reparse(compiled.clone()).expect("range admission");
    let snapshot = ui.reparse.source_snapshot().expect("active range display");
    assert_eq!(snapshot.range, Some(compiled.range));
    assert_eq!(snapshot.sources[0].name, "Alpha");
    assert!(
        ui.begin_reparse(compiled.clone()).is_none(),
        "duplicate range"
    );
    ui.screen = "history";
    let report = super::super::import_text::start_selected(
        &host,
        &compiled.path,
        Some(compiled.range),
        compiled.text,
        compiled.sources.as_ref(),
    )
    .await
    .expect("source range")
    .wait()
    .await
    .expect("range complete");
    assert_eq!((report.records_dispatched, report.damage), (3, 53));
    assert!(ui.receive_reparse(token, Ok(report), "complete", "cancelled"));
    assert!(ui.reparse.source_snapshot().is_none());
    cancel_selected_range(&host, &mut ui, &main).await;
    host.shutdown().await.expect("stop");
}
async fn cancel_selected_range(host: &Host, ui: &mut AppState, main: &Utf8Path) {
    use std::time::{Duration, Instant};
    std::fs::write(main, "1|1\n".repeat(100_000)).expect("long range");
    let draft = ui.reparse.draft.clone();
    let compiled = request(ui);
    let token = ui.begin_reparse(compiled.clone()).expect("range request");
    let snapshot = ui.reparse.source_snapshot().expect("range snapshot");
    let mut job = super::super::import_text::start_selected(
        host,
        &compiled.path,
        Some(compiled.range),
        compiled.text,
        compiled.sources.as_ref(),
    )
    .await
    .expect("cancel range job");
    let deadline = Instant::now() + Duration::from_secs(5);
    while job.progress().records_dispatched == 0 {
        tokio::select! {result=&mut job=>panic!("completed before cancel: {result:?}"),()=tokio::time::sleep(Duration::from_millis(1))=>{}}
        assert!(Instant::now() < deadline, "no range progress");
    }
    assert!(ui.cancel_reparse(token));
    assert!(!ui.cancel_reparse(token), "duplicate cancel");
    job.cancel();
    assert_eq!(ui.reparse.source_snapshot(), Some(snapshot));
    let result = job.wait().await;
    assert!(matches!(result, Err(aftercast::Error::Cancelled)));
    assert!(ui.receive_reparse(token, result, "complete", "cancelled"));
    assert_eq!(ui.reparse.draft, draft);
    assert_eq!(ui.message, "cancelled");
}
