use super::*;
pub(in crate::app) async fn test_contract(host: &aftercast::Host, ui: &mut AppState) {
    let fixture = camino::Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            aftercast::PluginConfig::builder()
                .path(fixture)
                .type_name("Aftercast.PluginFixture.NotificationPlugin")
                .build(),
        )
        .await
        .expect("synthetic notifications");
    host.initialize_plugin(plugin)
        .await
        .expect("initialize notifications");
    ui.notifications.page = Some(host.notifications(None, 16).await.expect("history"));
    let id = ui.notifications.page.as_ref().expect("page").items[0].id;
    let session = ui.begin_notification_request(true).expect("Show admitted");
    assert!(
        ui.begin_notification_request(true).is_none(),
        "duplicate click rejected"
    );
    assert!(
        ui.begin_notification_request(false).is_none(),
        "paging blocked until action completes"
    );
    ui.screen = "settings";
    assert!(ui.receive_notification_show(session, host.show_notification(id).await));
    assert_eq!(ui.screen, "settings", "completion must not navigate");
    assert!(ui.notifications.page.as_ref().expect("page").items[0].read);
    assert!(ui.notifications.page.as_ref().expect("page").items[0].can_show);
    let session = ui
        .begin_notification_request(true)
        .expect("intentional repeat");
    assert!(ui.receive_notification_show(session, host.show_notification(id).await));
    notification_stale_replies(ui, host, id).await;
}
async fn notification_stale_replies(ui: &mut AppState, host: &aftercast::Host, id: NotificationId) {
    let old = ui
        .begin_notification_request(true)
        .expect("old pending action");
    let response = host.show_notification(id).await;
    ui.revision.session += 1;
    ui.notifications = Default::default();
    ui.phase = Phase::Stopping;
    assert!(!ui.receive_notification_show(old, response));
    assert!(ui.phase == Phase::Stopping && ui.notifications.page.is_none());
    ui.phase = Phase::Ready;
    let current = ui
        .begin_notification_request(true)
        .expect("new worker action");
    assert!(!ui.finish_notification_request(old));
    assert!(ui.notifications.busy() && ui.phase == Phase::ShowingNotification);
    assert!(ui.receive_notification_show(current, Err(aftercast::Error::Busy)));
    assert!(ui.phase == Phase::Ready && !ui.notifications.busy() && ui.error.is_some());
}

fn config(root: &camino::Utf8Path) -> aftercast::HostConfig {
    let bridge = root.join("bridge");
    aftercast::extract_bridge(&bridge).expect("payload");
    let executable = std::env::current_exe().expect("test binary");
    let application = executable
        .parent()
        .and_then(std::path::Path::parent)
        .expect("target")
        .join("examples")
        .join(format!("test-application{}", std::env::consts::EXE_SUFFIX));
    aftercast::HostConfig::builder()
        .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
        .bridge_directory(bridge)
        .data_directory(root.join("state"))
        .application_executable(
            camino::Utf8PathBuf::try_from(application).expect("UTF8 executable"),
        )
        .build()
}
async fn fixture(host: &aftercast::Host) {
    let plugin = host.create_plugin(aftercast::PluginConfig::builder()
        .path(camino::Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll"))
        .type_name("Aftercast.PluginFixture.NotificationPlugin").build()).await.expect("fixture");
    host.initialize_plugin(plugin).await.expect("initialize");
}
async fn refresh(host: &aftercast::Host, ui: &mut AppState) {
    let token = ui.begin_notification_request(false).expect("load");
    let page = host.notifications(None, 16).await;
    assert!(ui.receive_notification_page(token, Movement::First, None, page));
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and synthetic Host; run mise run test:gui-state"]
async fn notification_bulk_ui_contract_guards_snapshot_counts_and_sessions() {
    let temp = tempfile::tempdir().expect("isolated data");
    let root = camino::Utf8Path::from_path(temp.path()).expect("UTF8 root");
    let config = config(root);
    let host = aftercast::Host::start(config.clone()).await.expect("host");
    fixture(&host).await;
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.revision.session = 1;
    refresh(&host, &mut ui).await;
    bulk_and_late_status(&host, &mut ui).await;
    overflow_and_session(&host, &mut ui).await;
    host.shutdown().await.expect("shutdown");
    let host = aftercast::Host::start(config).await.expect("restart");
    fixture(&host).await;
    ui.phase = Phase::Ready;
    refresh(&host, &mut ui).await;
    assert_eq!(
        ui.notifications
            .status
            .as_ref()
            .expect("fresh counts")
            .unread,
        5
    );
    assert_eq!(
        ui.notifications.page.as_ref().expect("fresh page").omitted,
        0
    );
    blink_persistence(&mut ui, root);
    host.shutdown().await.expect("cleanup");
}
async fn bulk_and_late_status(host: &aftercast::Host, ui: &mut AppState) {
    let page = ui.notifications.page.as_ref().expect("page").clone();
    let bound = page.status.snapshot_cursor;
    let repeat = page.items[0].id;
    let status_token = ui
        .begin_notification(Kind::Status)
        .expect("background refresh");
    let old_status = host.notification_status().await;
    let token = ui
        .begin_notification_bulk(bound)
        .expect("supersedes background read");
    assert!(
        ui.begin_notification_bulk(bound).is_none(),
        "duplicate bulk rejected"
    );
    assert!(ui.begin_notification_request(true).is_none());
    host.show_notification(repeat)
        .await
        .expect("arrival after accepted boundary");
    let change = host.mark_notifications_read(bound).await.expect("mark");
    ui.screen = "settings";
    assert!(ui.receive_notification_bulk(token, Ok(change.clone()), "marked"));
    assert_eq!(ui.screen, "settings");
    assert!(
        !ui.receive_notification_status(status_token, old_status),
        "obsolete status must not overwrite mutation"
    );
    assert!(!ui.receive_notification_bulk(token, Ok(change), "duplicate"));
    assert_eq!(ui.notifications.status.as_ref().expect("status").unread, 1);
    assert!(
        ui.begin_notification_bulk(bound).is_none(),
        "old rendered snapshot rejected"
    );
    let current = ui
        .begin_notification_request(false)
        .expect("new page request");
    assert!(!ui.receive_notification_bulk(
        token,
        Err(aftercast::Error::Busy),
        "duplicate late error"
    ));
    assert_eq!(ui.notifications.pending, Some(current));
    assert!(ui.receive_notification_page(
        current,
        Movement::First,
        None,
        host.notifications(None, 16).await
    ));
    clear_preserves_arrivals(host, ui, repeat).await;
}
async fn clear_preserves_arrivals(
    host: &aftercast::Host,
    ui: &mut AppState,
    repeat: NotificationId,
) {
    let page = ui.notifications.page.as_ref().expect("page").clone();
    let bound = page.status.snapshot_cursor;
    // Simulate paging history before clear; acknowledgement must reset it, not splice a stale page.
    ui.notifications.previous.push(None);
    ui.notifications.cursor = Some(page.next_cursor);
    let token = ui.begin_notification_bulk(bound).expect("clear");
    host.show_notification(repeat)
        .await
        .expect("another arrival outside clear bound");
    assert!(ui.receive_notification_bulk(
        token,
        host.clear_read_notifications(bound).await,
        "cleared"
    ));
    assert!(ui.notifications.previous.is_empty() && ui.notifications.cursor.is_none());
    let status = ui.notifications.status.as_ref().expect("status");
    assert_eq!((status.retained, status.unread, status.omitted), (2, 2, 0));
    let show = ui
        .begin_notification_request(true)
        .expect("old rendered Show");
    assert!(ui.receive_notification_show(show, host.show_notification(repeat).await));
    assert!(
        ui.error.is_some(),
        "cleared ID must visibly fail without retry"
    );
    assert_eq!(
        ui.notifications
            .page
            .as_ref()
            .expect("preserved page")
            .items
            .len(),
        2
    );
}
async fn overflow_and_session(host: &aftercast::Host, ui: &mut AppState) {
    fixture(host).await;
    refresh(host, ui).await;
    let fill = ui
        .notifications
        .page
        .as_ref()
        .expect("page")
        .items
        .iter()
        .find(|item| item.title == "fill")
        .expect("fill")
        .id;
    let token = ui.begin_notification_request(true).expect("Show");
    assert!(ui.receive_notification_show(token, host.show_notification(fill).await));
    assert!(
        ui.notifications.status.is_none(),
        "callback can change counts"
    );
    refresh(host, ui).await;
    let status = ui.notifications.status.as_ref().expect("status").clone();
    assert_eq!(status.unread, 256);
    assert!(status.omitted > 0, "overflow differs from explicit clear");
    let token = ui
        .begin_notification_bulk(status.snapshot_cursor)
        .expect("held mutation");
    let result = host.mark_notifications_read(status.snapshot_cursor).await;
    ui.revision.session += 1;
    ui.notifications = Default::default();
    ui.phase = Phase::Stopping;
    assert!(!ui.receive_notification_bulk(token, result, "late"));
    assert!(ui.notifications.page.is_none() && ui.phase == Phase::Stopping);
    ui.phase = Phase::Ready;
    let current = ui
        .begin_notification(Kind::Status)
        .expect("new session pending");
    assert!(!ui.receive_notification_bulk(token, Err(aftercast::Error::Busy), "old error"));
    assert_eq!(ui.notifications.pending, Some(current));
    assert!(ui.receive_notification_status(current, Err(aftercast::Error::Closed)));
    assert!(ui.notifications.status.is_none() && ui.notifications.status_error.is_some());
}
fn blink_persistence(ui: &mut AppState, root: &camino::Utf8Path) {
    use crate::config::ConfigFile;
    ui.file = Some(ConfigFile::load(root).expect("default config"));
    assert!(ui.settings.ui.notification_blink, "ACT default enabled");
    assert!(ui.toggle_notification_blink());
    let (_, file) = ui.begin_settings_save().expect("save");
    assert!(
        !ui.toggle_notification_blink(),
        "accepted save owns settings draft"
    );
    assert!(ui.begin_settings_save().is_none());
    let obstacle = root.join("obstacle");
    std::fs::write(&obstacle, "owned synthetic file").expect("obstacle");
    ui.receive_settings_save(file.save(&obstacle), "saved".into());
    assert!(!ui.settings.ui.notification_blink && !ui.saving);
    assert!(
        ui.file
            .as_ref()
            .expect("last acknowledged config")
            .settings
            .ui
            .notification_blink
    );
    let (destination, file) = ui.begin_settings_save().expect("retry");
    ui.screen = "analysis";
    ui.phase = Phase::Idle;
    ui.revision.session += 1;
    ui.notifications = Default::default();
    ui.receive_settings_save(file.save(&destination), "saved".into());
    assert_eq!(ui.screen, "analysis");
    assert!(
        !ui.file
            .as_ref()
            .expect("acknowledged config")
            .settings
            .ui
            .notification_blink
    );
    let restarted = ConfigFile::load(root).expect("restart config");
    assert!(!restarted.settings.ui.notification_blink);
    assert!(ui.notifications.page.is_none());
}

#[test]
fn notification_badge_bounds_and_quit_recovery_keep_counts_distinct() {
    assert_eq!(view::badge_text(0), None);
    assert_eq!(view::badge_text(1).as_deref(), Some("1"));
    assert_eq!(view::badge_text(9).as_deref(), Some("9"));
    assert_eq!(view::badge_text(10).as_deref(), Some("10+"));
    assert_eq!(view::badge_text(256).as_deref(), Some("10+"));
    let mut ui = AppState::new(".".into());
    ui.phase = Phase::Ready;
    ui.exit_requested = true;
    assert!(ui.begin_notification(Kind::Status).is_none());
    ui.exit_requested = false;
    assert!(
        ui.begin_notification(Kind::Status).is_some(),
        "cancelled Quit permits future refresh"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn notification_poll_survives_a_quit_tick_and_cancelled_exit() {
    let dom = VirtualDom::new(|| rsx! {});
    let mut signal = dom.in_scope(ScopeId::ROOT, || {
        let mut ui = AppState::new(".".into());
        ui.phase = Phase::Ready;
        ui.exit_requested = true;
        Signal::new(ui)
    });
    let polling = poll(signal);
    tokio::pin!(polling);
    tokio::select! {
        () = &mut polling => panic!("Quit must not permanently stop the app-owned polling loop"),
        () = tokio::time::sleep(std::time::Duration::from_millis(1100)) => {}
    }
    // This is the flag transition used when shutdown fails and the existing App remains mounted.
    signal.write().exit_requested = false;
    tokio::select! {
        () = &mut polling => panic!("Cancelled Quit must leave future refresh ticks alive"),
        () = tokio::time::sleep(std::time::Duration::from_millis(1100)) => {}
    }
    assert!(
        signal.read().notifications.pending.is_none(),
        "no Host means no speculative request"
    );
}
