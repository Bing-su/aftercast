use super::*;
use aftercast::{ImportProgress, ImportReport};
use model::{CloseAction, CloseState, Outcome, Token};
fn context() -> Context {
    Context {
        kind: Kind::File,
        viewpoint: None,
    }
}
fn report() -> ImportReport {
    ImportReport {
        lines_read: 2,
        records_dispatched: 2,
        encounters: 1,
        combatants: 2,
        damage: 40,
        healed: 0,
        companion_heads_excluded: 0,
    }
}
#[test]
fn progress_owner_rejects_old_actions_results_and_session_replacement() {
    let hub = Hub::default();
    let old = hub
        .begin(1, context(), ImportProgress::default())
        .expect("admission");
    assert!(hub.cancel(old));
    assert!(!hub.cancel(old));
    assert!(hub.should_cancel(old));
    assert!(
        hub.finish(old, &Ok(report())),
        "completion can win an already requested cancel"
    );
    assert_eq!(
        hub.snapshot().expect("retained completion").outcome,
        Outcome::Complete
    );
    assert!(!hub.publish(old, ImportProgress::default()));
    assert!(!hub.cancel(old));
    let next = hub
        .begin(1, context(), ImportProgress::default())
        .expect("next job");
    assert_ne!(old, next);
    assert!(!hub.cancel(old));
    assert!(!hub.finish(old, &Err(aftercast::Error::Cancelled)));
    assert!(hub.should_cancel(old));
    assert!(!hub.should_cancel(next));
    assert!(!hub.cancel(Token { session: 2, ..next }));
}
#[test]
fn progress_stop_and_root_close_reject_late_finish_and_new_admission() {
    let hub = Hub::default();
    let next = hub
        .begin(1, context(), ImportProgress::default())
        .expect("old Host job");
    hub.retire();
    assert!(hub.should_cancel(next));
    assert!(!hub.finish(next, &Ok(report())));
    assert_eq!(hub.snapshot().expect("retired").outcome, Outcome::Retired);
    let restarted = hub
        .begin(2, context(), ImportProgress::default())
        .expect("new Host");
    assert!(!hub.publish(next, ImportProgress::default()));
    assert!(!hub.should_cancel(restarted));
    hub.close();
    assert!(hub.closed());
    assert!(hub.should_cancel(restarted));
    assert!(hub.begin(3, context(), ImportProgress::default()).is_none());
}
#[test]
fn progress_retains_bounded_counts_and_never_copies_parser_error_text() {
    let hub = Hub::default();
    let token = hub
        .begin(
            1,
            Context {
                kind: Kind::Text,
                viewpoint: Some(aftercast::ImportCharacter::current()),
            },
            ImportProgress::default(),
        )
        .expect("text job");
    let progress = ImportProgress {
        lines_read: 4,
        records_dispatched: 3,
        bytes_read: 20,
        total_bytes: 100,
        companion_heads_excluded: 1,
    };
    assert!(hub.publish(token, progress));
    assert!(hub.finish(
        token,
        &Err(aftercast::Error::InvalidImportResponse(
            "SYNTHETIC_PRIVATE_SENTINEL".into()
        ))
    ));
    let snapshot = hub.snapshot().expect("failure projection");
    assert_eq!(snapshot.progress, progress);
    assert_eq!(snapshot.outcome, Outcome::Failed);
    assert!(snapshot.report.is_none());
    assert!(!format!("{snapshot:?}").contains("SYNTHETIC_PRIVATE_SENTINEL"));
    let token = hub
        .begin(1, context(), ImportProgress::default())
        .expect("retry");
    assert!(hub.finish(token, &Err(aftercast::Error::Cancelled)));
    assert_eq!(
        hub.snapshot().expect("canceled").outcome,
        Outcome::Cancelled
    );
}
#[test]
fn destroyed_window_failure_reopen_and_late_launches_never_cancel_an_import() {
    let hub = Hub::default();
    let token = hub
        .begin(1, context(), ImportProgress::default())
        .expect("job");
    let mut window = Launch::default();
    let (first, alive) = window.reserve().expect("first window");
    assert!(window.reserve().is_none(), "double click while queued");
    assert!(window.resolve(first, true));
    assert!(window.reserve().is_none(), "focus existing window");
    alive.store(false, Ordering::SeqCst);
    assert!(!hub.should_cancel(token), "closing is presentation only");
    let (next, _) = window.reserve().expect("reopen");
    assert!(!window.resolve(first, false));
    assert_eq!(window.pending, Some(next));
    assert!(window.resolve(next, false));
    assert!(window.error);
    assert!(!hub.should_cancel(token));
    let (retry, _) = window.reserve().expect("retry open failure");
    assert!(!window.error);
    assert!(!window.resolve(next, true));
    assert!(window.resolve(retry, true));
    hub.close();
    assert!(hub.should_cancel(token));
}

use aftercast::Host;
use camino::Utf8Path;
use std::{future::Future, pin::Pin, time::Duration};
async fn begin(
    host: &Host,
    mut state: Signal<AppState>,
    path: &Utf8Path,
) -> (aftercast::ImportJob, super::super::import_text::Token) {
    let selection = state.write().select_full_import().expect("picker request");
    let request = state
        .write()
        .admit_full_path(selection.token, path.to_owned())
        .expect("accepted path");
    let job = super::super::import_text::start_selected(
        host,
        path,
        None,
        request.options(),
        request.sources.as_ref(),
        request.viewpoint.as_ref(),
    )
    .await
    .expect("real job");
    (job, request.token)
}
async fn gate<F: Future<Output = Result<ImportReport, aftercast::Error>>>(
    root: &Utf8Path,
    waiting: &mut Pin<Box<F>>,
) {
    tokio::time::timeout(Duration::from_secs(5),async {
        while !root.join("state/character.ready").exists() {
            tokio::select! {result=waiting.as_mut()=>panic!("job finished before gate: {result:?}"),()=tokio::time::sleep(Duration::from_millis(1))=>{}}
        }
    }).await.expect("synthetic callback gate");
    // Poll even if the file was already ready before the first loop test.
    tokio::select! {result=waiting.as_mut()=>panic!("gate released too early: {result:?}"),()=tokio::time::sleep(Duration::from_millis(1))=>{}}
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and synthetic Host; run mise run test:gui-state"]
async fn progress_window_ui_contract_close_reopen_and_cancel_drain_real_job() {
    use super::super::import_text::character::tests::{choose, fixture};
    let temporary = tempfile::tempdir().expect("synthetic progress input");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    let (host, mut ui) = fixture(root).await;
    choose(&mut ui, "manual", "Target");
    let dom = VirtualDom::new(|| rsx! {});
    let mut state = dom.in_scope(ScopeId::ROOT, || Signal::new(ui));
    let path = root.join("main.log");
    std::fs::write(
        &path,
        "1|Target|Source|40|gate\n".to_owned() + &"2|Target|Source|1\n".repeat(20_000),
    )
    .expect("cancel file");
    let (job, request) = begin(&host, state, &path).await;
    let hub = state.peek().import_progress.hub.clone();
    let mut waiting = Box::pin(wait(state, job, 1));
    gate(root, &mut waiting).await;
    let frame = hub.snapshot().expect("actual owner published");
    assert_eq!(frame.context.kind, Kind::File);
    assert_eq!(
        frame
            .context
            .viewpoint
            .as_ref()
            .expect("fixed choice")
            .name(),
        "Target"
    );
    open(state);
    assert!(
        state.peek().import_progress.launch.error,
        "headless creation failure is visible without canceling"
    );
    assert!(!hub.should_cancel(frame.token));
    let mut launch = Launch::default();
    let (first, _) = launch.reserve().expect("queue window");
    assert!(launch.resolve(first, true));
    let mut closing = CloseState::default();
    assert_eq!(
        closing.request(hub.snapshot().as_ref()),
        CloseAction::Confirm
    );
    assert_eq!(closing.answer(&hub, frame.token, false), CloseAction::Keep);
    assert!(!hub.should_cancel(frame.token));
    assert!(
        launch.reserve().is_none(),
        "hidden/live singleton is reused rather than recreated"
    );
    state.write().screen = "history";
    assert_eq!(
        closing.request(hub.snapshot().as_ref()),
        CloseAction::Confirm
    );
    assert_eq!(closing.answer(&hub, frame.token, true), CloseAction::Hide);
    assert!(!hub.cancel(frame.token));
    poll_pending(&mut waiting).await;
    std::fs::write(root.join("state/character.release"), "release")
        .expect("release synthetic callback");
    let result = waiting.await;
    assert!(matches!(result, Err(aftercast::Error::Cancelled)));
    assert_eq!(
        hub.snapshot().expect("terminal").outcome,
        Outcome::Cancelled
    );
    assert!(
        state
            .write()
            .receive_full_import(request, result, "complete", "canceled")
    );
    assert!(
        host.encounters(0, 16)
            .await
            .expect("partial graph")
            .items
            .iter()
            .any(|e| e.status == aftercast::EncounterStatus::PartialImport && e.damage >= 40)
    );
    std::fs::write(&path, "1|Target|Source|7\n").expect("retry source");
    let (job, request) = begin(&host, state, &path).await;
    let result = wait(state, job, 1).await;
    assert!(
        state
            .write()
            .receive_full_import(request, result, "complete", "canceled")
    );
    let newest = hub.snapshot().expect("new job result");
    assert_eq!(newest.outcome, Outcome::Complete);
    assert_eq!(newest.report.expect("complete totals").damage, 7);
    assert!(!hub.cancel(frame.token));
    host.shutdown().await.expect("shutdown");
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and synthetic Host; run mise run test:gui-state"]
async fn progress_window_ui_contract_failure_and_root_exit_keep_owner_cleanup() {
    use super::super::import_text::character::tests::fixture;
    let temporary = tempfile::tempdir().expect("synthetic progress failure");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    let (host, ui) = fixture(root).await;
    let dom = VirtualDom::new(|| rsx! {});
    let mut state = dom.in_scope(ScopeId::ROOT, || Signal::new(ui));
    let path = root.join("main.log");
    const PRIVATE: &str = "SYNTHETIC_PRIVATE_PROGRESS_SENTINEL";
    std::fs::write(&path, format!("1|Source|Target|11\n{PRIVATE}\n")).expect("partial input");
    let (job, request) = begin(&host, state, &path).await;
    let hub = state.peek().import_progress.hub.clone();
    let result = wait(state, job, 1).await;
    assert!(matches!(result, Err(aftercast::Error::Managed { .. })));
    let snapshot = hub.snapshot().expect("failure frame");
    assert_eq!(snapshot.outcome, Outcome::Failed);
    assert!(!format!("{snapshot:?}").contains(PRIVATE));
    assert!(
        state
            .write()
            .receive_full_import(request, result, "complete", "canceled")
    );
    std::fs::write(
        &path,
        "1|Source|Target|11|gate\n".to_owned() + &"2|Source|Target|1\n".repeat(20_000),
    )
    .expect("exit source");
    let (job, request) = begin(&host, state, &path).await;
    let mut waiting = Box::pin(wait(state, job, 1));
    gate(root, &mut waiting).await;
    let token = hub.snapshot().expect("running").token;
    state.write().exit_requested = true;
    state.write().import_progress.close();
    state.write().revision.session += 1;
    state.write().import_text.reset_session();
    state.write().phase = super::super::Phase::Stopping;
    poll_pending(&mut waiting).await;
    std::fs::write(root.join("state/character.release"), "release").expect("release");
    let (result, shutdown) = tokio::join!(waiting, host.shutdown());
    shutdown.expect("concurrent owner shutdown drained");
    assert!(matches!(result, Err(aftercast::Error::Cancelled)));
    assert_eq!(hub.snapshot().expect("root exit").outcome, Outcome::Retired);
    assert!(!hub.cancel(token));
    assert!(
        !state
            .write()
            .receive_full_import(request, result, "late", "late")
    );
}

async fn poll_pending<F: Future<Output = Result<ImportReport, aftercast::Error>>>(
    waiting: &mut Pin<Box<F>>,
) {
    tokio::select! {result=waiting.as_mut()=>panic!("synthetic callback was not held: {result:?}"),()=tokio::time::sleep(Duration::from_millis(150))=>{}}
}

#[test]
fn root_close_rejects_a_late_open_before_context_lookup_or_error_mutation() {
    let dom = VirtualDom::new(|| rsx! {});
    let state = dom.in_scope(ScopeId::ROOT, || {
        Signal::new(AppState::new("/synthetic".into()))
    });
    state.peek().import_progress.close();
    open(state);
    assert!(!state.peek().import_progress.launch.error);
    assert!(state.peek().import_progress.launch.pending.is_none());
}

#[test]
fn act_close_question_no_keeps_and_yes_cancels_only_the_confirmed_job() {
    let hub = Hub::default();
    let token = hub
        .begin(1, context(), ImportProgress::default())
        .expect("job");
    let mut close = CloseState::default();
    assert_eq!(close.request(hub.snapshot().as_ref()), CloseAction::Confirm);
    assert_eq!(close.answer(&hub, token, false), CloseAction::Keep);
    assert!(!hub.should_cancel(token));
    assert_eq!(close.request(hub.snapshot().as_ref()), CloseAction::Confirm);
    assert_eq!(close.answer(&hub, token, true), CloseAction::Hide);
    assert!(hub.should_cancel(token));
    assert_eq!(
        close.answer(&hub, token, true),
        CloseAction::Ignored,
        "duplicate old Yes"
    );
    hub.finish(token, &Err(aftercast::Error::Cancelled));
    assert_eq!(
        close.request(hub.snapshot().as_ref()),
        CloseAction::Hide,
        "terminal closes without a question"
    );
}
#[test]
fn act_close_confirmation_survives_completion_but_never_retargets_a_new_import() {
    let hub = Hub::default();
    let old = hub
        .begin(1, context(), ImportProgress::default())
        .expect("first");
    let mut close = CloseState::default();
    close.request(hub.snapshot().as_ref());
    hub.finish(old, &Ok(report()));
    assert_eq!(close.answer(&hub, old, true), CloseAction::Hide);
    let next = hub
        .begin(1, context(), ImportProgress::default())
        .expect("next");
    close.request(hub.snapshot().as_ref());
    let replacement = hub
        .begin(2, context(), ImportProgress::default())
        .expect("replacement");
    assert_eq!(close.answer(&hub, next, true), CloseAction::Ignored);
    assert!(!hub.should_cancel(replacement));
    close.request(hub.snapshot().as_ref());
    hub.begin(2, context(), ImportProgress::default())
        .expect("later");
    close.sync(hub.snapshot().as_ref());
    assert!(close.confirmation.is_none());
    assert_eq!(close.answer(&hub, replacement, true), CloseAction::Ignored);
    hub.retire();
    assert_eq!(close.request(hub.snapshot().as_ref()), CloseAction::Hide);
    assert_eq!(close.request(None), CloseAction::Hide);
}

#[test]
fn stale_close_yes_and_no_cannot_clear_a_new_native_question_or_change_visibility() {
    let hub = Hub::default();
    let old = hub
        .begin(1, context(), ImportProgress::default())
        .expect("A");
    let mut closing = CloseState::default();
    closing.request(hub.snapshot().as_ref());
    let next = hub
        .begin(1, context(), ImportProgress::default())
        .expect("B");
    closing.request(hub.snapshot().as_ref());
    let restore = AtomicBool::new(true);
    for yes in [true, false] {
        assert_eq!(
            window::answer_visibility(&mut closing, &hub, old, yes, &restore),
            None
        );
        assert!(restore.load(Ordering::SeqCst));
        assert_eq!(closing.confirmation, Some(next));
        assert!(!hub.should_cancel(next));
    }
    assert_eq!(
        window::answer_visibility(&mut closing, &hub, next, false, &restore),
        Some(true)
    );
    assert!(!restore.load(Ordering::SeqCst));
    assert!(!hub.should_cancel(next));
}
#[test]
fn a_valid_close_yes_hides_and_root_close_never_resurrects_a_window() {
    let hub = Hub::default();
    let token = hub
        .begin(1, context(), ImportProgress::default())
        .expect("job");
    let mut closing = CloseState::default();
    closing.request(hub.snapshot().as_ref());
    let restore = AtomicBool::new(true);
    assert_eq!(
        window::answer_visibility(&mut closing, &hub, token, true, &restore),
        Some(false)
    );
    assert!(!restore.load(Ordering::SeqCst));
    assert!(hub.should_cancel(token));
    closing.request(hub.snapshot().as_ref());
    hub.close();
    restore.store(true, Ordering::SeqCst);
    assert_eq!(
        window::answer_visibility(&mut closing, &hub, token, false, &restore),
        None
    );
    assert!(restore.load(Ordering::SeqCst));
}
