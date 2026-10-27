use super::*;
use aftercast::{Host, HostConfig, PluginConfig};
use camino::Utf8Path;
use dioxus::prelude::*;
use std::{
    sync::{Mutex, atomic::AtomicUsize},
    time::Duration,
};

struct MemoryReader {
    content: Mutex<Result<Option<String>, ()>>,
    calls: Arc<AtomicUsize>,
}
impl Reader for MemoryReader {
    fn read_text(&self) -> Result<Option<String>, ()> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.content.lock().map_err(|_poisoned| ())?.clone()
    }
}
fn adapter(content: Result<Option<String>, ()>) -> (Adapter, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    (
        Adapter::new(
            MemoryReader {
                content: Mutex::new(content),
                calls: calls.clone(),
            },
            true,
        ),
        calls,
    )
}
async fn paste(ui: &mut AppState, adapter: &Adapter) -> Token {
    let view = ui.clipboard_view();
    let token = ui
        .begin_clipboard_read(view)
        .expect("explicit supplied read");
    assert!(ui.begin_clipboard_read(view).is_none(), "duplicate read");
    assert!(!ui.clear_clipboard_preview(view));
    let result = adapter.read().await;
    assert!(ui.receive_clipboard_read(token, result));
    token
}
#[tokio::test(flavor = "current_thread")]
async fn supplied_clipboard_adapter_preserves_draft_and_has_bounded_unicode_preview() {
    let mut ui = AppState::new("/isolated".into());
    let content = "😀한글\r\n".repeat(200);
    let (source, calls) = adapter(Ok(Some(content.clone())));
    assert_eq!(
        calls.load(Ordering::Relaxed),
        0,
        "construction/render setup never reads"
    );
    paste(&mut ui, &source).await;
    let draft = ui.clipboard.draft.clone().expect("snapshot");
    assert_eq!(draft.text.as_ref(), content);
    assert_eq!(draft.preview.chars().count(), 512);
    assert_eq!(draft.characters, content.chars().count());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    for (input, failure) in [
        (Ok(None), Failure::Unsupported),
        (Err(()), Failure::Unavailable),
        (Ok(Some(String::new())), Failure::Empty),
        (
            Ok(Some("한".repeat(MAX_CLIPBOARD_IMPORT_BYTES / 3 + 1))),
            Failure::TooLarge,
        ),
    ] {
        let (source, _) = adapter(input);
        paste(&mut ui, &source).await;
        assert_eq!(ui.clipboard.error, Some(failure));
        assert!(Arc::ptr_eq(
            &ui.clipboard.draft.as_ref().expect("kept draft").text,
            &draft.text
        ));
        assert!(ui.error.is_none());
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let unavailable = Adapter::new(
        MemoryReader {
            content: Mutex::new(Ok(None)),
            calls: calls.clone(),
        },
        false,
    );
    assert!(matches!(
        unavailable.read().await,
        Err(Failure::Unavailable)
    ));
    assert_eq!(
        calls.load(Ordering::Relaxed),
        0,
        "unavailable provider is never invoked"
    );
    let view = ui.clipboard_view();
    ui.screen = "history";
    assert!(ui.clear_clipboard_preview(view));
    assert!(!ui.clear_clipboard_preview(view), "obsolete clear event");
    assert!(ui.clipboard.draft.is_none());
}
struct BlockingReader {
    entered: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
}
impl Reader for BlockingReader {
    fn read_text(&self) -> Result<Option<String>, ()> {
        self.entered.store(true, Ordering::Release);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !self.release.load(Ordering::Acquire) {
            if std::time::Instant::now() >= deadline {
                return Err(());
            }
            std::thread::yield_now();
        }
        Ok(Some("synthetic stale data".into()))
    }
}
#[tokio::test(flavor = "current_thread")]
async fn supplied_clipboard_read_is_single_flight_and_stale_session_cannot_replace_preview() {
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let source = Adapter::new(
        BlockingReader {
            entered: entered.clone(),
            release: release.clone(),
        },
        true,
    );
    let mut ui = AppState::new("/isolated".into());
    ui.clipboard.draft = Some(Snapshot::new("previous draft".into()).expect("old snapshot"));
    let token = ui.begin_clipboard_read(ui.clipboard_view()).expect("read");
    let background = tokio::spawn({
        let source = source.clone();
        async move { source.read().await }
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !entered.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("in-memory reader entered");
    ui.revision.session += 1;
    ui.clipboard.reset_session();
    ui.phase = Phase::Stopping;
    assert!(matches!(source.read().await, Err(Failure::Busy)));
    release.store(true, Ordering::Release);
    let result = background.await.expect("synthetic task");
    assert!(!ui.receive_clipboard_read(token, result));
    assert_eq!(
        ui.clipboard
            .draft
            .as_ref()
            .expect("old retained")
            .text
            .as_ref(),
        "previous draft"
    );
    assert!(ui.phase == Phase::Stopping);
    assert!(!source.busy());
    let fresh = AppState::new(ui.root.clone());
    assert!(
        fresh.clipboard.draft.is_none(),
        "preview is not persisted across app launches"
    );
}
#[tokio::test(flavor = "current_thread")]
async fn quitting_pending_supplied_read_returns_to_existing_idle_or_host_stop_path() {
    for previous in [Phase::Idle, Phase::Ready] {
        quitting_read(previous).await;
    }
}

async fn quitting_read(previous: Phase) {
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let source = Adapter::new(
        BlockingReader {
            entered: entered.clone(),
            release: release.clone(),
        },
        true,
    );
    let mut ui = AppState::new("/isolated".into());
    ui.phase = previous;
    let token = ui.begin_clipboard_read(ui.clipboard_view()).expect("read");
    let background = tokio::spawn({
        let source = source.clone();
        async move { source.read().await }
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !entered.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("pending memory reader");
    assert!(
        !ui.abandon_clipboard_read_for_exit(),
        "requires an actual Quit request"
    );
    ui.exit_requested = true;
    assert!(ui.abandon_clipboard_read_for_exit());
    assert!(!ui.clipboard.active());
    assert!(
        ui.phase == previous,
        "Ready routes to Stop; Idle routes to window close"
    );
    assert!(
        !ui.abandon_clipboard_read_for_exit(),
        "duplicate Quit cleanup"
    );
    release.store(true, Ordering::Release);
    let result = background.await.expect("memory reader finishes");
    assert!(!ui.receive_clipboard_read(token, result));
    assert!(ui.exit_requested && ui.phase == previous);
    assert!(ui.clipboard.draft.is_none());
}

fn configuration(root: &Utf8Path) -> HostConfig {
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
        .dotnet_root(std::env::var("DOTNET_ROOT").expect("runtime"))
        .bridge_directory(bridge)
        .data_directory(root.join("state"))
        .application_executable(camino::Utf8PathBuf::try_from(application).expect("UTF8 app"))
        .build()
}
async fn initialize(host: &Host, ui: &mut AppState) {
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = host
        .create_plugin(
            PluginConfig::builder()
                .path(path.clone())
                .type_name("Aftercast.PluginFixture.ClipboardImportPlugin")
                .build(),
        )
        .await
        .expect("fixture");
    let status = host.initialize_plugin(id).await.expect("parser");
    ui.plugins.push(super::super::LoadedPlugin {
        path: path.into_string(),
        id,
        status,
        info: host.plugin_status(id).await.expect("status"),
    });
}
async fn execute(host: &Host, ui: &mut AppState) -> Token {
    let view = ui.clipboard_view();
    let (token, snapshot) = ui.begin_clipboard_import(view).expect("explicit Import");
    assert!(ui.begin_clipboard_import(view).is_none());
    assert!(ui.begin_clipboard_read(ui.clipboard_view()).is_none());
    assert!(!ui.clear_clipboard_preview(ui.clipboard_view()));
    assert!(!ui.can_full_import());
    ui.screen = "history";
    let result = host.import_clipboard_text(snapshot.text.as_ref()).await;
    assert!(ui.receive_clipboard_import(token, result, "complete", "canceled"));
    token
}
#[derive(Clone)]
struct LogSink(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for LogSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_poisoned| std::io::Error::other("test log capture poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and synthetic Host; run mise run test:gui-state"]
async fn clipboard_ui_contract_owns_preview_and_redacts_failures_without_os_clipboard_access() {
    let temporary = tempfile::tempdir().expect("isolated clipboard GUI controller");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8 root");
    let host = Host::start(configuration(root))
        .await
        .expect("actual isolated worker");
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.revision.session = 1;
    initialize(&host, &mut ui).await;
    ui.import_path = "unrelated-file.log".into();
    assert!(
        ui.edit_import_text(ui.text_view(), "encoding", "ascii")
            .expect("raw option")
    );
    let raw_options = ui.import_text.options();
    let (source, calls) = adapter(Ok(Some("1|10|한글\r\nU|20|日本語".into())));
    let read = paste(&mut ui, &source).await;
    assert!(!ui.receive_clipboard_read(read, Snapshot::new("stale snapshot".into())));
    let draft = ui.clipboard.draft.clone().expect("preview staged");
    let completed = execute(&host, &mut ui).await;
    assert_eq!(ui.message, "complete · 2 / 2 · 1");
    assert_eq!(
        calls.load(Ordering::Relaxed),
        1,
        "Import uses the preview, not another provider read"
    );
    assert!(Arc::ptr_eq(
        &draft.text,
        &ui.clipboard.draft.as_ref().expect("draft kept").text
    ));
    assert_eq!(ui.import_path, "unrelated-file.log");
    assert_eq!(ui.import_text.options(), raw_options);
    assert_eq!(ui.screen, "history");
    assert!(!ui.receive_clipboard_import(
        completed,
        Err(aftercast::Error::Cancelled),
        "late",
        "late"
    ));
    let logs = Arc::new(Mutex::new(Vec::new()));
    let sink = logs.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(move || LogSink(sink.clone()))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::info!("synthetic clipboard GUI log sink ready");
    const SECRET: &str = "SYNTHETIC_CLIPBOARD_PRIVATE_SENTINEL";
    let (source, _) = adapter(Ok(Some(format!("1|7\n{SECRET}"))));
    paste(&mut ui, &source).await;
    execute(&host, &mut ui).await;
    assert_eq!(ui.clipboard.error, Some(Failure::Import));
    assert!(ui.error.is_none());
    assert!(!ui.message.contains(SECRET));
    assert!(!String::from_utf8_lossy(&logs.lock().expect("captured logs")).contains(SECRET));
    assert!(
        String::from_utf8_lossy(&logs.lock().expect("captured logs")).contains("log sink ready")
    );
    assert!(
        ui.clipboard
            .draft
            .as_ref()
            .expect("failed draft retained")
            .text
            .contains(SECRET)
    );
    assert!(
        host.encounters(0, 16)
            .await
            .expect("partial graph")
            .items
            .iter()
            .any(|row| row.status == aftercast::EncounterStatus::PartialImport && row.damage == 7)
    );
    signal_cancel(&host, root, &ui).await;
    host.shutdown().await.expect("shutdown");
}
async fn signal_cancel(host: &Host, root: &Utf8Path, ui: &AppState) {
    let dom = VirtualDom::new(|| rsx! {});
    let mut signal = dom.in_scope(ScopeId::ROOT, || {
        let mut state = AppState::new(root.to_owned());
        state.phase = Phase::Ready;
        state.revision.session = ui.revision.session;
        state.plugins = ui.plugins.clone();
        Signal::new(state)
    });
    let (source, _) = adapter(Ok(Some("1|1|gate\n".to_owned() + &"1|1\n".repeat(20_000))));
    let view = signal.peek().clipboard_view();
    let read = signal
        .write()
        .begin_clipboard_read(view)
        .expect("read admission");
    let result = source.read().await;
    assert!(signal.write().receive_clipboard_read(read, result));
    let view = signal.peek().clipboard_view();
    let (token, snapshot) = signal
        .write()
        .begin_clipboard_import(view)
        .expect("import admission");
    let job = host
        .start_clipboard_import(snapshot.text.as_ref())
        .await
        .expect("actual job");
    tokio::time::timeout(Duration::from_secs(5), async {
        while !root.join("state/clipboard.ready").exists() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("synthetic callback gate");
    signal.write().screen = "analysis";
    assert!(signal.write().cancel_clipboard_import(token));
    assert!(!signal.write().cancel_clipboard_import(token));
    std::fs::write(root.join("state/clipboard.release"), "release")
        .expect("release only synthetic callback");
    let result = super::super::wait_import(signal, job, token.session).await;
    assert!(matches!(result, Err(aftercast::Error::Cancelled)));
    assert!(
        signal
            .write()
            .receive_clipboard_import(token, result, "complete", "canceled")
    );
    assert_eq!(signal.peek().screen, "analysis");
    assert_eq!(signal.peek().message, "canceled");
    assert!(signal.peek().clipboard.draft.is_some());
    let view = signal.peek().clipboard_view();
    let (token, snapshot) = signal
        .write()
        .begin_clipboard_import(view)
        .expect("second admission");
    let job = host
        .start_clipboard_import(snapshot.text.as_ref())
        .await
        .expect("second job");
    {
        let mut state = signal.write();
        state.revision.session += 1;
        state.clipboard.reset_session();
        state.phase = Phase::Stopping;
        state.progress = Default::default();
    }
    let result = super::super::wait_import(signal, job, token.session).await;
    assert!(
        !signal
            .write()
            .receive_clipboard_import(token, result, "late", "late")
    );
    assert!(!signal.write().cancel_clipboard_import(token));
    assert!(signal.peek().phase == Phase::Stopping);
    assert!(signal.peek().clipboard.draft.is_some());
}
