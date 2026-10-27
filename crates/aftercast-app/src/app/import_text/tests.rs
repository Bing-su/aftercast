use super::*;
use aftercast::{Host, HostConfig, PluginConfig};

fn config(root: &Utf8Path) -> HostConfig {
    let bridge = root.join("bridge");
    aftercast::extract_bridge(&bridge).expect("payload");
    let executable = std::env::current_exe().expect("test binary");
    let application = executable
        .parent()
        .and_then(std::path::Path::parent)
        .expect("target")
        .join("examples")
        .join(format!("test-application{}", std::env::consts::EXE_SUFFIX));
    HostConfig::builder()
        .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
        .bridge_directory(bridge)
        .data_directory(root.join("state"))
        .application_executable(Utf8PathBuf::try_from(application).expect("UTF8 executable"))
        .build()
}
async fn parser(host: &Host, ui: &mut AppState) {
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = host
        .create_plugin(
            PluginConfig::builder()
                .path(path.clone())
                .type_name("Aftercast.PluginFixture.KoreanEncodingPlugin")
                .build(),
        )
        .await
        .expect("fixture");
    let status = host.initialize_plugin(id).await.expect("initialize");
    ui.plugins.push(super::super::LoadedPlugin {
        path: path.into_string(),
        id,
        status,
        info: host.plugin_status(id).await.expect("status"),
    });
}
fn edit(ui: &mut AppState, field: &str, value: &str) {
    assert!(
        ui.edit_import_text(ui.text_view(), field, value)
            .expect("valid editor choice")
    );
}
fn accept(ui: &mut AppState, input: &Utf8Path) -> FullRequest {
    let selection = ui.select_full_import().expect("selection");
    assert!(
        ui.select_full_import().is_none(),
        "duplicate picker admission"
    );
    assert!(
        !ui.edit_import_text(ui.text_view(), "encoding", "ascii")
            .expect("ignored busy edit")
    );
    let request = ui
        .admit_full_path(selection.token, input.to_owned())
        .expect("native admission");
    assert!(
        ui.admit_full_path(selection.token, input.to_owned())
            .is_none(),
        "duplicate native admission"
    );
    request
}
async fn full(host: &Host, ui: &mut AppState, input: &Utf8Path) -> Token {
    let request = accept(ui, input);
    let result = start(host, input, None, request.options())
        .await
        .expect("start")
        .wait()
        .await;
    assert!(ui.receive_full_import(request.token, result, "complete", "cancelled"));
    request.token
}
async fn originals(host: &Host, offset: u32) -> Vec<String> {
    let id = host
        .encounters(offset, 1)
        .await
        .expect("graph")
        .items
        .first()
        .expect("encounter")
        .id;
    host.encounter_logs(id, 0, 16)
        .await
        .expect("logs")
        .expect("scope")
        .page
        .items
        .into_iter()
        .map(|item| item.text)
        .collect()
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and synthetic Host; run mise run test:gui-state"]
async fn import_text_ui_contract_preserves_choices_and_binds_full_requests() {
    let temporary = tempfile::tempdir().expect("isolated data");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8 root");
    let configuration = config(root);
    let host = Host::start(configuration.clone()).await.expect("host");
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.revision.session = 1;
    parser(&host, &mut ui).await;
    let input = root.join("source.log");
    // Independently encoded Windows Korean fixture bytes for 한글.
    std::fs::write(&input, b"0|100|\xc7\xd1\xb1\xdb\n").expect("Korean source");
    assert!(
        ui.import_text.options().is_none(),
        "initial UI preserves plugin inheritance"
    );
    full(&host, &mut ui, &input).await;
    assert_eq!(originals(&host, 0).await, ["0|100|한글"]);
    explicit_and_failures(&host, &mut ui, &input).await;
    let stale = cancelled_and_stale(&host, &mut ui, &input).await;
    let draft = ui.import_text.draft.clone();
    host.shutdown().await.expect("stop");
    let host = Host::start(configuration).await.expect("new Host");
    ui.phase = Phase::Ready;
    parser(&host, &mut ui).await;
    assert_eq!(ui.import_text.draft, draft, "Host restart retains choices");
    let selection = ui.select_full_import().expect("new picker");
    assert!(ui.admit_full_path(stale, input.to_owned()).is_none());
    assert!(!ui.abandon_full_selection(stale));
    assert!(
        ui.import_text.active(),
        "late old picker must not clear new selection"
    );
    assert!(ui.abandon_full_selection(selection.token));
    std::fs::write(&input, "0|1|日本語🙂\n").expect("UTF8 source");
    edit(&mut ui, "encoding", "utf8");
    full(&host, &mut ui, &input).await;
    assert_eq!(originals(&host, 0).await, ["0|1|日本語🙂"]);
    assert!(
        AppState::new(root.to_owned())
            .import_text
            .options()
            .is_none(),
        "new App resets to inheritance"
    );
    host.shutdown().await.expect("cleanup");
}
async fn explicit_snapshot(host: &Host, ui: &mut AppState, input: &Utf8Path) -> Draft {
    edit(ui, "encoding", "utf8");
    let draft = ui.import_text.draft.clone();
    let previous = ui.text_view();
    let request = accept(ui, input);
    let held = start(host, input, None, request.options())
        .await
        .expect("start");
    ui.screen = "settings";
    // A model-only external edit cannot change an already submitted request's value snapshot.
    ui.import_text.draft.encoding = Some(ImportEncoding::WindowsKorean);
    assert!(ui.receive_full_import(request.token, held.wait().await, "complete", "cancelled"));
    assert!(
        ui.error.is_some(),
        "explicit strict UTF8 overrides the Korean plugin"
    );
    assert_eq!(ui.screen, "settings");
    ui.import_text.draft = draft.clone();
    assert!(
        !ui.edit_import_text(previous, "encoding", "ascii")
            .expect("stale view")
    );
    assert_eq!(ui.import_text.draft, draft);
    draft
}
async fn explicit_and_failures(host: &Host, ui: &mut AppState, input: &Utf8Path) {
    let draft = explicit_snapshot(host, ui, input).await;
    std::fs::write(input, b"0|100|valid\n1|200|\xff\n").expect("invalid suffix");
    let before = host.encounters(0, 256).await.expect("before").total;
    let token = full(host, ui, input).await;
    assert_eq!(ui.import_text.draft, draft, "failure keeps draft");
    assert_eq!(originals(host, before).await, ["0|100|valid"]);
    assert!(!ui.receive_full_import(token, Err(aftercast::Error::Busy), "late", "late"));
    edit(ui, "invalid", "replace");
    full(host, ui, input).await;
    assert!(
        originals(host, before + 1)
            .await
            .get(1)
            .expect("replaced row")
            .contains('\u{fffd}')
    );
    let unchanged = ui.import_text.draft.clone();
    assert_eq!(
        ui.edit_import_text(ui.text_view(), "encoding", "unknown"),
        Err("import-text-invalid-choice")
    );
    assert_eq!(ui.import_text.draft, unchanged);
    failed_path(host, ui, input).await;
    bom_policy(host, ui, input).await;
}
async fn failed_path(host: &Host, ui: &mut AppState, input: &Utf8Path) {
    let draft = ui.import_text.draft.clone();
    let missing = input.with_file_name("missing.log");
    let request = accept(ui, &missing);
    let result = match start(host, &missing, None, request.options()).await {
        Ok(job) => job.wait().await,
        Err(error) => Err(error),
    };
    assert!(ui.receive_full_import(request.token, result, "complete", "cancelled"));
    assert!(ui.error.is_some());
    assert_eq!(ui.import_path, missing.as_str());
    assert_eq!(ui.import_text.draft, draft);
}
async fn bom_policy(host: &Host, ui: &mut AppState, input: &Utf8Path) {
    edit(ui, "invalid", "strict");
    let mut bytes = vec![0xff, 0xfe];
    bytes.extend(
        "0|10|한글日本語🙂\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes),
    );
    std::fs::write(input, bytes).expect("UTF16 BOM");
    full(host, ui, input).await;
    assert!(
        ui.error.is_none(),
        "BOM detection inherits explicit strictness"
    );
    edit(ui, "bom", "false");
    full(host, ui, input).await;
    assert!(
        ui.error.is_some(),
        "strict UTF8 with other BOM detection off rejects UTF16"
    );
    edit(ui, "bom", "true");
}
fn cancel_picker(ui: &mut AppState) {
    let selection = ui.select_full_import().expect("picker");
    assert!(ui.current_full_selection(selection.token));
    ui.exit_requested = true;
    assert!(
        !ui.current_full_selection(selection.token),
        "Quit before task starts must not open a new dialog"
    );
    ui.exit_requested = false;
    assert!(ui.current_full_selection(selection.token));
    assert!(ui.abandon_full_selection(selection.token));
    assert!(ui.phase == Phase::Ready);
    assert!(!ui.abandon_full_selection(selection.token));
    assert!(!ui.current_full_selection(selection.token));
}
async fn cancelled_and_stale(host: &Host, ui: &mut AppState, input: &Utf8Path) -> Token {
    cancel_picker(ui);
    std::fs::write(input, "1|1|日本語🙂\n".repeat(100_000)).expect("large source");
    let request = accept(ui, input);
    let mut job = start(host, input, None, request.options())
        .await
        .expect("job");
    tokio::time::timeout(std::time::Duration::from_secs(10),async {
        while job.progress().records_dispatched==0 {
            tokio::select! { result=&mut job => panic!("finished too early: {result:?}"), ()=tokio::time::sleep(std::time::Duration::from_millis(1))=>{} }
        }
    }).await.expect("admitted progress");
    let draft = ui.import_text.draft.clone();
    job.cancel();
    assert!(ui.receive_full_import(request.token, job.wait().await, "complete", "cancelled"));
    assert_eq!(ui.message, "cancelled");
    assert_eq!(ui.import_text.draft, draft);
    let current = accept(ui, input);
    assert!(!ui.receive_full_import(request.token, Err(aftercast::Error::Closed), "old", "old"));
    assert!(ui.import_text.active() && ui.phase == Phase::Importing);
    let job = start(host, input, None, current.options())
        .await
        .expect("new job");
    job.cancel();
    let response = job.wait().await;
    ui.revision.session += 1;
    ui.import_text.reset_session();
    ui.plugins.clear();
    ui.phase = Phase::Stopping;
    assert!(!ui.receive_full_import(current.token, response, "late", "late"));
    assert!(ui.phase == Phase::Stopping && ui.import_text.draft == draft);
    assert!(
        !ui.edit_import_text(current.token, "encoding", "ascii")
            .expect("old session event")
    );
    current.token
}
