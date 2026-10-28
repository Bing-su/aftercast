mod common;
use aftercast::{
    Error, Host, PluginConfig, TableCellError, TableLevel, TableQuery, TableScope, TableSort,
    TableSortError,
};
use camino::Utf8Path;
use common::{Deployment, deployment};
fn plugin(name: &str) -> PluginConfig {
    PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll",
        ))
        .type_name(format!("Aftercast.PluginFixture.{name}"))
        .build()
}
async fn query(host: &Host, scope: TableScope, columns: &[&str]) -> TableQuery {
    let catalog = host.table_columns(scope.level()).await.expect("catalog");
    TableQuery {
        scope,
        revision: catalog.revision,
        columns: columns.iter().map(|key| (*key).to_owned()).collect(),
        offset: 0,
        limit: 16,
        sort: None,
    }
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn plugin_cells_sorting_and_revisions_stay_owned_by_the_worker(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    assert!(
        host.table_columns(TableLevel::Combatant)
            .await
            .expect("cold catalog")
            .columns
            .is_empty()
    );
    let id = host
        .create_plugin(plugin("ColumnsPlugin"))
        .await
        .expect("plugin");
    host.initialize_plugin(id).await.expect("initialize");
    let input = deployment.directory().join("table source.log");
    std::fs::write(&input, "0|100\n0|300\n1|200\n").expect("synthetic source");
    host.import_log(&input).await.expect("import");
    let encounter = host
        .encounters(0, 16)
        .await
        .expect("encounters")
        .items
        .first()
        .expect("encounter")
        .id;
    let snapshot = host
        .capture_category_actions(encounter, "Source", "Damage", true)
        .await
        .expect("capture")
        .expect("category");
    let root_query = query(
        &host,
        TableScope::Categories {
            encounter,
            combatant: "Source".into(),
        },
        &["fixture"],
    )
    .await;
    let root_page = host
        .lookup_table_rows(snapshot, &root_query, None, None)
        .await
        .expect("lookup peer table");
    assert_eq!(root_page.total, 1);
    assert_eq!(
        root_page
            .rows
            .first()
            .and_then(|row| row.cells.first())
            .and_then(|cell| cell.text.as_deref()),
        Some("600")
    );
    let attack_query = query(
        &host,
        TableScope::Attacks {
            encounter,
            combatant: "Source".into(),
            category: "Damage".into(),
        },
        &["fixture"],
    )
    .await;
    let attacks = host
        .lookup_table_rows(snapshot, &attack_query, Some("Target"), None)
        .await
        .expect("lookup skills");
    assert_eq!(attacks.total, 2);
    host.capture_category_actions(encounter, "Source", "Damage", true)
        .await
        .expect("replacement");
    host.lookup_table_rows(snapshot, &root_query, None, None)
        .await
        .expect_err("stale lookup");
    let scopes = [
        TableScope::Encounters,
        TableScope::Combatants(encounter),
        TableScope::Categories {
            encounter,
            combatant: "Source".into(),
        },
        TableScope::Attacks {
            encounter,
            combatant: "Source".into(),
            category: "Damage".into(),
        },
        TableScope::Actions {
            encounter,
            combatant: "Source".into(),
            category: "Damage".into(),
            attack: "All".into(),
        },
    ];
    for scope in scopes {
        let request = query(&host, scope, &["fixture"]).await;
        let page = host
            .table_rows(&request)
            .await
            .expect("table")
            .expect("scope");
        assert!(!page.rows.is_empty());
        assert!(
            page.rows
                .iter()
                .flat_map(|row| &row.cells)
                .all(|cell| cell.error.is_none())
        );
    }
    let actors = TableScope::Combatants(encounter);
    let mut probe = query(&host, actors.clone(), &["probe"]).await;
    let page = host
        .table_rows(&probe)
        .await
        .expect("probe")
        .expect("scope");
    assert_eq!(
        page.rows
            .iter()
            .map(|row| row.cells.first().expect("cell").text.as_deref())
            .collect::<Vec<_>>(),
        [Some("1"), Some("2")]
    );
    let mut request = query(&host, actors.clone(), &["fixture", "bad", "null"]).await;
    request.sort = Some(TableSort {
        primary: "fixture".into(),
        secondary: None,
        descending: true,
    });
    let page = host
        .table_rows(&request)
        .await
        .expect("sorted table")
        .expect("scope");
    assert!(page.sort_error.is_none());
    let cell = page
        .rows
        .first()
        .expect("actor")
        .cells
        .first()
        .expect("cell");
    assert_eq!(cell.text.as_deref(), Some("Source:600"));
    assert_eq!(cell.foreground_argb, Some(-65536));
    for row in &page.rows {
        assert_eq!(
            row.cells.get(1).expect("throwing formatter").error,
            Some(TableCellError::Column)
        );
        assert_eq!(
            row.cells.get(2).expect("null formatter").error,
            Some(TableCellError::Column)
        );
    }
    request.sort = Some(TableSort {
        primary: "bad".into(),
        secondary: None,
        descending: false,
    });
    let failed = host
        .table_rows(&request)
        .await
        .expect("sort fallback")
        .expect("scope");
    assert_eq!(failed.sort_error, Some(TableSortError::Callback));
    assert_eq!(
        failed.rows.iter().map(|row| row.index).collect::<Vec<_>>(),
        [0, 1]
    );
    request.limit = 1;
    request.offset = 1;
    assert_eq!(
        host.table_rows(&request)
            .await
            .expect("page")
            .expect("scope")
            .rows
            .len(),
        1
    );
    let missing = query(
        &host,
        TableScope::Actions {
            encounter,
            combatant: "Source".into(),
            category: "Damage".into(),
            attack: "missing".into(),
        },
        &["fixture"],
    )
    .await;
    assert!(
        host.table_rows(&missing)
            .await
            .expect("missing scope")
            .is_none()
    );
    let empty = query(
        &host,
        TableScope::Attacks {
            encounter,
            combatant: "Source".into(),
            category: "Healing".into(),
        },
        &["fixture"],
    )
    .await;
    let empty_page = host
        .table_rows(&empty)
        .await
        .expect("valid empty table")
        .expect("existing category");
    assert_eq!(empty_page.total, 0);
    assert!(empty_page.rows.is_empty());
    busy_and_abandoned(&host, &input, &probe).await;
    let mutation = query(&host, actors.clone(), &["mutate"]).await;
    host.table_rows(&mutation)
        .await
        .expect_err("self-unregistering formatter invalidates whole reply");
    host.table_rows(&probe)
        .await
        .expect_err("old catalog revision rejected");
    probe = query(&host, actors.clone(), &["probe"]).await;
    assert!(
        host.table_rows(&probe)
            .await
            .expect("fresh catalog")
            .is_some()
    );
    host.deinitialize_plugin(id).await.expect("disable");
    assert!(
        host.table_columns(TableLevel::Combatant)
            .await
            .expect("removed registrations")
            .columns
            .is_empty()
    );
    host.table_rows(&probe)
        .await
        .expect_err("disabled definitions cannot be read");
    let next = host
        .create_plugin(plugin("ColumnsPlugin"))
        .await
        .expect("fresh instance");
    host.initialize_plugin(next).await.expect("reenable");
    let fresh = query(&host, actors, &["probe"]).await;
    assert_eq!(
        host.table_rows(&fresh)
            .await
            .expect("fresh callbacks")
            .expect("scope")
            .rows
            .first()
            .expect("row")
            .cells
            .first()
            .expect("cell")
            .text
            .as_deref(),
        Some("1")
    );
    host.shutdown().await.expect("shutdown");
    let restarted = Host::start(deployment.config()).await.expect("restart");
    assert!(matches!(
        restarted.table_rows(&fresh).await,
        Err(Error::InvalidTableRequest(_))
    ));
    let mut old_scope = fresh.clone();
    old_scope.revision = restarted
        .table_columns(TableLevel::Combatant)
        .await
        .expect("new revision")
        .revision;
    assert!(matches!(
        restarted.table_rows(&old_scope).await,
        Err(Error::InvalidTableRequest(_))
    ));
    restarted.shutdown().await.expect("restart cleanup");
}
async fn busy_and_abandoned(host: &Host, input: &Utf8Path, probe: &TableQuery) {
    use std::{future::Future, task::Poll};
    std::fs::write(input, "0|1\n".repeat(100_000)).expect("long source");
    let job = host.start_import(input).await.expect("active import");
    assert!(matches!(host.table_rows(probe).await, Err(Error::Busy)));
    assert!(matches!(
        host.table_columns(TableLevel::Combatant).await,
        Err(Error::Busy)
    ));
    job.cancel();
    assert!(matches!(job.wait().await, Err(Error::Cancelled)));
    let mut pending = Box::pin(host.table_rows(probe));
    std::future::poll_fn(|context| {
        if let Poll::Ready(result) = pending.as_mut().poll(context) {
            result.expect("early table reply");
        }
        Poll::Ready(())
    })
    .await;
    host.diagnostics()
        .await
        .expect("FIFO admission/completion barrier");
    drop(pending);
    assert!(
        host.table_rows(probe)
            .await
            .expect("reuse after dropped reply")
            .is_some()
    );
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and synthetic blocked formatter; run mise run test:plugins"]
async fn blocked_managed_formatter_is_retired_by_existing_shutdown_boundary(
    deployment: Deployment,
) {
    use std::time::{Duration, Instant};
    let mut config = deployment.config();
    config.shutdown_timeout = Duration::from_millis(150);
    let host = Host::start(config).await.expect("host");
    let id = host
        .create_plugin(plugin("StuckColumnsPlugin"))
        .await
        .expect("plugin");
    host.initialize_plugin(id).await.expect("initialize");
    let input = deployment.directory().join("blocked.log");
    std::fs::write(&input, "0|1\n").expect("source");
    host.import_log(&input).await.expect("import");
    let encounter = host
        .encounters(0, 16)
        .await
        .expect("graph")
        .items
        .first()
        .expect("encounter")
        .id;
    let request = query(&host, TableScope::Combatants(encounter), &["fixture"]).await;
    let handle = host.handle();
    let pending = tokio::spawn(async move { handle.table_rows(&request).await });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !deployment.state.join("columns.entered").exists() {
        assert!(Instant::now() < deadline, "formatter did not start");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(3), host.shutdown())
            .await
            .expect("shutdown deadline"),
        Err(Error::ShutdownTimeout(_))
    ));
    tokio::time::timeout(Duration::from_secs(1), pending)
        .await
        .expect("pending query released")
        .expect("query task")
        .expect_err("terminated query must fail");
    let next = Host::start(deployment.config())
        .await
        .expect("worker reaped/state lease released");
    next.shutdown().await.expect("fresh worker shutdown");
}
