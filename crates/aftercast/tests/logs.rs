mod common;

use aftercast::{
    Error, Host, LogLoss, LogSubscription, LogSubscriptionConfig, LogTimeKind, PluginConfig,
};
use camino::Utf8Path;
use common::{Deployment, deployment};

#[rstest::rstest]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
fn bounded_subscribers_report_loss_without_interrupting_combat_or_owning_clr_memory(
    deployment: Deployment,
) {
    let directory = deployment.directory();
    let config = || deployment.config();
    let mut host = Host::start(config()).expect("start subscription host");
    let subscription = host
        .subscribe_logs(LogSubscriptionConfig::builder().max_records(1).build())
        .expect("subscribe before plugin initialization");
    assert!(matches!(
        host.subscribe_logs(LogSubscriptionConfig::builder().build()),
        Err(Error::Managed { command: 9, .. })
    ));
    let plugin = host.create_plugin(PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll"))
        .type_name("Aftercast.PluginFixture.ImportPlugin").build()).expect("construct parser fixture");
    host.initialize_plugin(plugin)
        .expect("initialize parser fixture");
    let path = directory.join("input 한글.log");
    check_count_overflow(&mut host, &path, subscription);
    let (retained, large_line) = check_large_record(&mut host, &path);
    let subscription = check_oversized_record(&mut host, &path);
    host.stop().expect("stop with active subscription");

    let mut restarted = Host::start(config()).expect("restart process-owned managed session");
    let next = restarted
        .subscribe_logs(LogSubscriptionConfig::builder().build())
        .expect("clear permits subscription after restart");
    assert!(matches!(
        restarted.drain_logs(subscription),
        Err(Error::InvalidLogRequest(_))
    ));
    assert!(
        restarted
            .drain_logs(next)
            .expect("new subscription is empty")
            .records
            .is_empty()
    );
    assert_eq!(retained.as_ref(), large_line.as_bytes());
    restarted.stop().expect("stop restarted session");
}

fn check_count_overflow(host: &mut Host, path: &Utf8Path, subscription: LogSubscription) {
    std::fs::write(path, "0|100\n2|300\n").expect("write two source records");
    assert_eq!(
        host.import_log(path)
            .expect("core continues through overflow")
            .damage,
        400
    );
    let batch = host
        .drain_logs(subscription)
        .expect("drain retained record and loss");
    assert_eq!(
        batch.dropped,
        Some(LogLoss {
            first_sequence: 1,
            last_sequence: 1,
            count: 1
        })
    );
    assert_eq!(batch.records.len(), 1);
    let record = batch.records.first().expect("retained newest record");
    assert_eq!(record.sequence, 2);
    assert!(record.is_import);
    assert_eq!(
        record.source_path.as_ref(),
        path.canonicalize_utf8()
            .expect("canonical source path")
            .as_path()
    );
    assert_eq!(record.original_line.as_ref(), b"2|300");
    assert_eq!(record.timestamp.kind, LogTimeKind::Utc);
    assert_eq!(record.timestamp.ticks, 621_355_968_020_000_000);
    assert_eq!(record.timestamp.utc_ticks, Some(record.timestamp.ticks));
    assert!(
        host.drain_logs(subscription)
            .expect("empty subsequent drain")
            .records
            .is_empty()
    );
    host.unsubscribe_logs(subscription).expect("unsubscribe");
    assert!(matches!(
        host.drain_logs(subscription),
        Err(Error::Managed { command: 9, .. })
    ));
}

fn check_large_record(host: &mut Host, path: &Utf8Path) -> (bytes::Bytes, String) {
    let subscription = host
        .subscribe_logs(LogSubscriptionConfig::builder().max_bytes(200_000).build())
        .expect("subscribe for larger records");
    let large_line = format!("0|100|{}", "한".repeat(30_000));
    std::fs::write(path, &large_line).expect("write record larger than the initial native batch");
    assert_eq!(
        host.import_log(path).expect("import large source").damage,
        100
    );
    let batch = host
        .drain_logs(subscription)
        .expect("capacity retry consumes exactly once");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(batch.dropped, None);
    let retained = batch
        .records
        .first()
        .expect("large owned record")
        .original_line
        .clone();
    assert_eq!(retained.as_ref(), large_line.as_bytes());
    assert!(
        host.drain_logs(subscription)
            .expect("large record was consumed once")
            .records
            .is_empty()
    );
    host.unsubscribe_logs(subscription)
        .expect("retire large-record subscription");
    (retained, large_line)
}

fn check_oversized_record(host: &mut Host, path: &Utf8Path) -> LogSubscription {
    let subscription = host
        .subscribe_logs(LogSubscriptionConfig::builder().max_bytes(256).build())
        .expect("subscribe with tight byte bound");
    std::fs::write(path, format!("0|100\n2|300|{}\n4|50\n", "x".repeat(500)))
        .expect("write oversized middle record");
    assert_eq!(
        host.import_log(path)
            .expect("oversized subscriber record does not stop aggregation")
            .damage,
        450
    );
    let batch = host
        .drain_logs(subscription)
        .expect("drain exact missing prefix");
    assert_eq!(
        batch.dropped,
        Some(LogLoss {
            first_sequence: 1,
            last_sequence: 2,
            count: 2
        })
    );
    assert_eq!(
        batch
            .records
            .first()
            .expect("continued after oversized record")
            .original_line
            .as_ref(),
        b"4|50"
    );
    subscription
}
