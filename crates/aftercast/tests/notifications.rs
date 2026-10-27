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
