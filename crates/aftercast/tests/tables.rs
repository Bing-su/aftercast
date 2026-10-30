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

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn comma_option_reaches_plugin_formatting_without_rewriting_literal_cells(
    deployment: Deployment,
) {
    let input = deployment.directory().join("commas.log");
    std::fs::write(&input, "0|1234\n").expect("synthetic");
    for (commas, expected) in [(true, "1,234"), (false, "1234")] {
        let config = aftercast::HostConfig::builder()
            .application_executable(common::application())
            .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
            .bridge_directory(&deployment.bridge)
            .data_directory(&deployment.state)
            .main_table_commas(commas)
            .build();
        let host = Host::start(config).await.expect("start");
        let id = host
            .create_plugin(plugin("CommasPlugin"))
            .await
            .expect("plugin");
        host.initialize_plugin(id).await.expect("initialize");
        host.import_log(&input).await.expect("import");
        let encounter = host.encounters(0, 16).await.expect("encounters").items[0].id;
        let scope = TableScope::Actions {
            encounter,
            combatant: "Source".into(),
            category: "Damage".into(),
            attack: "Strike".into(),
        };
        let request = query(&host, scope, &["format", "literal"]).await;
        let page = host
            .table_rows(&request)
            .await
            .expect("rows")
            .expect("scope");
        assert_eq!(page.rows[0].cells[0].text.as_deref(), Some(expected));
        assert_eq!(page.rows[0].cells[1].text.as_deref(), Some("custom:1,234"));
        host.shutdown().await.expect("stop");
    }
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn zone_scope_uses_owned_identity_even_for_equal_zone_names(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    let id = host
        .create_plugin(plugin("ZoneFixturePlugin"))
        .await
        .expect("plugin");
    host.initialize_plugin(id).await.expect("initialize");
    let page = host.encounters(0, 16).await.expect("all");
    assert_eq!(page.total, 4);
    assert!(
        page.items
            .iter()
            .all(|encounter| encounter.zone == "Same Zone")
    );
    let zones = host.zones(0, 16).await.expect("zone list");
    let matching: Vec<_> = zones
        .items
        .iter()
        .filter(|zone| zone.name == "Same Zone")
        .collect();
    assert_eq!(matching.len(), 3);
    assert_ne!(matching[0].id, matching[1].id);
    assert_ne!(matching[1].id, matching[2].id);
    let empty = matching[2].id;
    assert_eq!(matching[2].encounters, 0);
    assert_eq!(
        host.encounters_in_zone(empty, 0, 1)
            .await
            .expect("empty zone")
            .expect("exists")
            .total,
        0
    );
    let empty_query = query(&host, TableScope::ZoneId(empty), &["fixture"]).await;
    assert_eq!(
        host.table_rows(&empty_query)
            .await
            .expect("table")
            .expect("exists")
            .total,
        0
    );
    let first = page.items[0].id;
    let second = page.items[2].id;
    let a = host
        .zone_encounters(first, 0, 1)
        .await
        .expect("zone")
        .expect("anchor");
    let b = host
        .zone_encounters(second, 0, 1)
        .await
        .expect("zone")
        .expect("anchor");
    assert_eq!((a.total, b.total), (2, 2));
    assert_eq!(a.items[0].id, first);
    assert_eq!(b.items[0].id, second);
    assert_ne!(
        host.zone_encounters(first, 1, 1)
            .await
            .expect("next")
            .expect("zone")
            .items[0]
            .id,
        first
    );
    let qa = query(&host, TableScope::Zone(first), &["fixture"]).await;
    let qb = query(&host, TableScope::Zone(second), &["fixture"]).await;
    assert_eq!(
        host.table_rows(&qa)
            .await
            .expect("table")
            .expect("zone")
            .total,
        2
    );
    assert_eq!(
        host.table_rows(&qb)
            .await
            .expect("table")
            .expect("zone")
            .rows[0]
            .encounter,
        second
    );
    host.deinitialize_plugin(id).await.expect("remove zones");
    assert!(
        host.encounters_in_zone(empty, 0, 1)
            .await
            .expect("removed empty zone")
            .is_none()
    );
    assert!(
        host.zone_encounters(first, 0, 1)
            .await
            .expect("removed")
            .is_none()
    );
    host.table_rows(&qa)
        .await
        .expect_err("removed column revision");
    host.shutdown().await.expect("stop");
    let fresh = Host::start(deployment.config()).await.expect("restart");
    fresh
        .encounters_in_zone(empty, 0, 1)
        .await
        .expect_err("old zone session");
    fresh
        .zone_encounters(first, 0, 1)
        .await
        .expect_err("old session");
    fresh.table_rows(&qa).await.expect_err("old table session");
    fresh.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn published_main_sort_is_observed_by_plugin_and_auto_reads_do_not_change_it(
    deployment: Deployment,
) {
    let host = Host::start(deployment.config()).await.expect("host");
    let plugin_id = host
        .create_plugin(plugin("SortKeysPlugin"))
        .await
        .expect("plugin");
    host.initialize_plugin(plugin_id).await.expect("initialize");
    let input = deployment.directory().join("sort-key-source.log");
    std::fs::write(&input, "0|100\n1|200\n").expect("synthetic source");
    host.import_log(&input).await.expect("import");
    let encounter = host.encounters(0, 1).await.expect("encounters").items[0].id;
    let mut request = query(&host, TableScope::Combatants(encounter), &["sort-keys"]).await;
    request.sort = Some(TableSort {
        primary: "fixture".into(),
        secondary: Some("probe".into()),
        descending: false,
    });
    let page = host
        .main_table_rows(&request, false, true)
        .await
        .expect("publish")
        .expect("scope");
    assert_eq!(page.rows[0].cells[0].text.as_deref(), Some("fixture|probe"));
    request.sort = Some(TableSort {
        primary: "probe".into(),
        secondary: None,
        descending: false,
    });
    let page = host
        .main_table_rows(&request, false, false)
        .await
        .expect("automatic read")
        .expect("scope");
    assert_eq!(page.rows[0].cells[0].text.as_deref(), Some("fixture|probe"));
    let catalog = host
        .table_columns(TableLevel::Combatant)
        .await
        .expect("click catalog");
    let click = host
        .click_main_sort(&catalog, "fixture", false)
        .await
        .expect("click");
    assert_eq!(click.primary, "fixture");
    assert_eq!(click.secondary.as_deref(), Some("fixture"));
    assert!(click.descending);
    let click = host
        .click_main_sort(&catalog, "probe", click.descending)
        .await
        .expect("different click");
    assert_eq!(click.secondary.as_deref(), Some("fixture"));
    assert!(click.descending);
    let page = host
        .main_table_rows(&request, click.descending, false)
        .await
        .expect("clicked page")
        .expect("scope");
    assert_eq!(page.rows[0].cells[0].text.as_deref(), Some("probe|fixture"));
    host.deinitialize_plugin(plugin_id).await.expect("remove");
    host.click_main_sort(&catalog, "fixture", false)
        .await
        .expect_err("stale click catalog");
    host.main_table_rows(&request, false, true)
        .await
        .expect_err("stale catalog");
    host.shutdown().await.expect("stop");
    let restarted = Host::start(deployment.config()).await.expect("restart");
    restarted
        .click_main_sort(&catalog, "fixture", false)
        .await
        .expect_err("old catalog session");
    restarted.shutdown().await.expect("restart stop");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn actor_color_classification_survives_worker_boundary(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    let id = host
        .create_plugin(plugin("ClassificationPlugin"))
        .await
        .expect("plugin");
    host.initialize_plugin(id).await.expect("initialize");
    let input = deployment.directory().join("synthetic-colors.log");
    std::fs::write(&input, "0|100\n").expect("synthetic input");
    host.import_log(input).await.expect("import");
    let encounter = host
        .encounters(0, 16)
        .await
        .expect("encounters")
        .items
        .first()
        .expect("encounter")
        .id;
    let request = query(&host, TableScope::Combatants(encounter), &["fixture"]).await;
    let page = host
        .table_rows(&request)
        .await
        .expect("table")
        .expect("page");
    let own = page
        .rows
        .iter()
        .find(|row| {
            row.combatant
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case("Source"))
        })
        .expect("own actor");
    assert!(own.is_self && own.is_ally);
    assert_eq!(
        own.cells.first().expect("plugin cell").foreground_argb,
        Some(-65536)
    );
    let enemy = page
        .rows
        .iter()
        .find(|row| {
            row.combatant
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case("Target"))
        })
        .expect("target");
    assert!(!enemy.is_self && !enemy.is_ally);
    let handle = host.handle();
    host.shutdown().await.expect("shutdown");
    handle
        .table_rows(&request)
        .await
        .expect_err("stale session");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn color_scalars_reach_plugin_events_on_owner_and_retire_with_session(
    deployment: Deployment,
) {
    use aftercast::ColorScalars;
    let host = Host::start(deployment.config()).await.expect("host");
    host.set_color_scalars(ColorScalars {
        invert: true,
        luminosity: 25,
        saturation: -75,
    })
    .await
    .expect("initial");
    let id = host
        .create_plugin(plugin("ColorScalarPlugin"))
        .await
        .expect("plugin");
    host.initialize_plugin(id).await.expect("init");
    let input = deployment.directory().join("synthetic-color-events.log");
    std::fs::write(&input, "0|100\n").expect("synthetic input");
    host.import_log(input).await.expect("import");
    let encounter = host
        .encounters(0, 16)
        .await
        .expect("encounters")
        .items
        .first()
        .expect("encounter")
        .id;
    let request = query(&host, TableScope::Combatants(encounter), &["color-probe"]).await;
    let page = host
        .table_rows(&request)
        .await
        .expect("table")
        .expect("scope");
    assert_eq!(
        page.rows
            .first()
            .expect("row")
            .cells
            .first()
            .expect("cell")
            .text
            .as_deref(),
        Some("True|0.25|-0.75|")
    );
    host.set_color_scalars(ColorScalars {
        invert: false,
        luminosity: 10,
        saturation: 20,
    })
    .await
    .expect("changed");
    let page = host
        .table_rows(&request)
        .await
        .expect("table")
        .expect("scope");
    assert_eq!(
        page.rows
            .first()
            .expect("row")
            .cells
            .first()
            .expect("cell")
            .text
            .as_deref(),
        Some("False|0.1|0.2|invert,number,number")
    );
    host.set_color_scalars(ColorScalars {
        invert: false,
        luminosity: 10,
        saturation: 20,
    })
    .await
    .expect("unchanged");
    let page = host
        .table_rows(&request)
        .await
        .expect("table")
        .expect("scope");
    assert_eq!(
        page.rows
            .first()
            .expect("row")
            .cells
            .first()
            .expect("cell")
            .text
            .as_deref(),
        Some("False|0.1|0.2|invert,number,number")
    );
    host.set_color_scalars(ColorScalars {
        invert: false,
        luminosity: 12,
        saturation: 20,
    })
    .await
    .expect("plugin reentry");
    let page = host
        .table_rows(&request)
        .await
        .expect("table")
        .expect("scope");
    assert_eq!(
        page.rows
            .first()
            .expect("row")
            .cells
            .first()
            .expect("cell")
            .text
            .as_deref(),
        Some("False|0.34|0.2|invert,number,number,number,number")
    );
    host.set_color_scalars(ColorScalars {
        invert: false,
        luminosity: 56,
        saturation: 20,
    })
    .await
    .expect_err("subscriber failure is explicit");
    let page = host
        .table_rows(&request)
        .await
        .expect("table")
        .expect("scope");
    assert!(
        page.rows
            .first()
            .expect("row")
            .cells
            .first()
            .expect("cell")
            .text
            .as_deref()
            .is_some_and(|v| v.starts_with("False|0.56|0.2|"))
    );
    host.set_color_scalars(ColorScalars {
        invert: false,
        luminosity: 10,
        saturation: 20,
    })
    .await
    .expect("recovery");
    let controls = aftercast::ColorControls {
        ally: Some(0x010203),
        personal: None,
    };
    host.set_color_controls(controls)
        .await
        .expect("control colors");
    let control_request = query(
        &host,
        TableScope::Combatants(encounter),
        &["color-controls-probe"],
    )
    .await;
    let control_page = host
        .table_rows(&control_request)
        .await
        .expect("table")
        .expect("scope");
    assert_eq!(
        control_page
            .rows
            .first()
            .expect("row")
            .cells
            .first()
            .expect("cell")
            .text
            .as_deref(),
        Some("-16711165|0|ally:-16711165,personal:0")
    );
    host.set_color_controls(controls)
        .await
        .expect("unchanged controls");
    let control_page = host
        .table_rows(&control_request)
        .await
        .expect("table")
        .expect("scope");
    assert_eq!(
        control_page
            .rows
            .first()
            .expect("row")
            .cells
            .first()
            .expect("cell")
            .text
            .as_deref(),
        Some("-16711165|0|ally:-16711165,personal:0")
    );
    host.set_color_controls(aftercast::ColorControls {
        ally: Some(0x1000000),
        personal: None,
    })
    .await
    .expect_err("invalid RGB");
    host.set_color_scalars(ColorScalars {
        invert: false,
        luminosity: 101,
        saturation: 0,
    })
    .await
    .expect_err("invalid");
    host.set_color_scalars(ColorScalars {
        invert: true,
        luminosity: 13,
        saturation: -75,
    })
    .await
    .expect("reentrant precision");
    host.set_color_controls(aftercast::ColorControls {
        ally: Some(0x112233),
        personal: None,
    })
    .await
    .expect("reentrant alpha");
    let runtime = host
        .handle()
        .runtime_colors()
        .await
        .expect("owner snapshot");
    assert!((runtime.luminosity - 0.345).abs() < 0.00001);
    assert_eq!(runtime.ally, Some(0x4008090a));
    host.deinitialize_plugin(id).await.expect("deinitialize");
    host.set_color_scalars(ColorScalars::default())
        .await
        .expect("no retired subscriber");
    let old = host.handle();
    host.shutdown().await.expect("stop");
    old.set_color_scalars(ColorScalars::default())
        .await
        .expect_err("stale session");
}
