mod common;
use aftercast::{Error, Host, PluginConfig};
use camino::Utf8Path;
use common::{Deployment, deployment};

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn notification_callbacks_keep_owner_identity_and_expire_with_history_and_session(
    deployment: Deployment,
) {
    let host = Host::start(deployment.config()).await.expect("host");
    let assembly = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let fixture = deployment.directory().join("Aftercast.PluginFixture.dll");
    std::fs::copy(assembly, &fixture).expect("isolate synthetic fixture");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(fixture)
                .type_name("Aftercast.PluginFixture.NotificationPlugin")
                .build(),
        )
        .await
        .expect("fixture");
    host.initialize_plugin(plugin).await.expect("initialize");
    let page = host.notifications(None, 16).await.expect("history");
    assert_eq!(page.items.len(), 5);
    let repeat = page.items[0].id;
    for _ in 0..2 {
        let item = host.show_notification(repeat).await.expect("repeat Show");
        assert!(item.read && item.can_show && item.show_error.is_none());
    }
    let page_after = host
        .notifications(Some(page.next_cursor), 16)
        .await
        .expect("new callback entries");
    assert_eq!(
        page_after
            .items
            .iter()
            .map(|item| item.message.as_str())
            .collect::<Vec<_>>(),
        ["1", "2"]
    );
    let disposed = host
        .show_notification(page.items[1].id)
        .await
        .expect("disposed");
    assert!(disposed.read && !disposed.can_show && disposed.show_error.is_none());
    let failed = host
        .show_notification(page.items[2].id)
        .await
        .expect("throw caught");
    assert!(failed.read && !failed.can_show && failed.show_error.is_some());
    host.show_notification(failed.id)
        .await
        .expect("disabled callback is not retried");
    let attempts = host
        .notifications(Some(page_after.next_cursor), 16)
        .await
        .expect("attempt history");
    assert_eq!(attempts.items.len(), 1);
    assert_eq!(attempts.items[0].title, "failure-attempt");
    let inert = host
        .show_notification(page.items[3].id)
        .await
        .expect("read no-handler");
    assert!(inert.read && !inert.can_show);
    host.show_notification(page.items[4].id)
        .await
        .expect("fill history");
    assert!(
        host.notifications(None, 16)
            .await
            .expect("bounded history")
            .omitted
            > 0
    );
    host.show_notification(repeat)
        .await
        .expect_err("evicted identity rejected");
    let closed = host.handle();
    host.shutdown().await.expect("shutdown");
    closed
        .show_notification(failed.id)
        .await
        .expect_err("closed session rejects Show");
    let restarted = Host::start(deployment.config()).await.expect("restart");
    assert!(matches!(
        restarted.show_notification(repeat).await,
        Err(Error::InvalidNotificationRequest(_))
    ));
    assert!(matches!(
        restarted.notifications(Some(page.next_cursor), 16).await,
        Err(Error::InvalidNotificationRequest(_))
    ));
    assert!(
        restarted
            .notifications(None, 16)
            .await
            .expect("new session")
            .items
            .is_empty()
    );
    restarted.shutdown().await.expect("stop");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn notification_bulk_uses_whole_snapshot_and_preserves_new_arrivals(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    let assembly = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(assembly)
                .type_name("Aftercast.PluginFixture.NotificationPlugin")
                .build(),
        )
        .await
        .expect("fixture");
    host.initialize_plugin(plugin).await.expect("initialize");
    let page = host.notifications(None, 1).await.expect("one visible item");
    assert_eq!(page.items.len(), 1);
    assert_eq!((page.status.retained, page.status.unread), (5, 5));
    let snapshot = page.status.snapshot_cursor;
    let repeat = page.items[0].id;
    host.show_notification(repeat)
        .await
        .expect("synthetic addition after snapshot");
    let marked = host
        .mark_notifications_read(snapshot)
        .await
        .expect("mark whole snapshot");
    assert_eq!(marked.changed, 4, "whole snapshot, not one displayed item");
    assert_eq!(
        (marked.page.status.retained, marked.page.status.unread),
        (6, 1)
    );
    assert!(
        marked
            .page
            .items
            .iter()
            .find(|item| item.title == "shown")
            .is_some_and(|item| !item.read)
    );
    assert_eq!(
        host.mark_notifications_read(snapshot)
            .await
            .expect("duplicate mark")
            .changed,
        0
    );
    host.show_notification(repeat)
        .await
        .expect("mark did not disable callback");
    let cleared = host
        .clear_read_notifications(snapshot)
        .await
        .expect("clear old read entries");
    assert_eq!(cleared.changed, 5);
    assert_eq!(
        (cleared.page.status.retained, cleared.page.status.unread),
        (2, 2)
    );
    assert_eq!(
        cleared.page.omitted, 0,
        "intentional removal is not omission"
    );
    assert_eq!(
        host.clear_read_notifications(snapshot)
            .await
            .expect("duplicate clear")
            .changed,
        0
    );
    host.show_notification(repeat)
        .await
        .expect_err("cleared callback identity expires");
    let later = host
        .notifications(Some(page.next_cursor), 16)
        .await
        .expect("cursor remains valid across removal");
    assert_eq!(later.items.len(), 2);
    let status = host
        .notification_status()
        .await
        .expect("whole-history badge");
    assert_eq!((status.retained, status.unread), (2, 2));
    host.mark_notifications_read(status.snapshot_cursor)
        .await
        .expect("read rest");
    let empty = host
        .clear_read_notifications(status.snapshot_cursor)
        .await
        .expect("clear rest");
    assert_eq!(empty.changed, 2);
    assert!(empty.page.items.is_empty());
    assert_eq!(
        (empty.page.status.retained, empty.page.status.unread),
        (0, 0)
    );
    host.shutdown().await.expect("shutdown");
    let restarted = Host::start(deployment.config()).await.expect("restart");
    assert!(matches!(
        restarted.mark_notifications_read(snapshot).await,
        Err(Error::InvalidNotificationRequest(_))
    ));
    assert!(matches!(
        restarted.clear_read_notifications(snapshot).await,
        Err(Error::InvalidNotificationRequest(_))
    ));
    let fresh = restarted
        .notification_status()
        .await
        .expect("fresh empty snapshot");
    assert_eq!(
        restarted
            .mark_notifications_read(fresh.snapshot_cursor)
            .await
            .expect("empty mark")
            .changed,
        0
    );
    assert_eq!(
        restarted
            .clear_read_notifications(fresh.snapshot_cursor)
            .await
            .expect("empty clear")
            .changed,
        0
    );
    restarted.shutdown().await.expect("stop");
}
