use super::*;
use aftercast::{Host, HostConfig, PluginConfig, TableLevel, TableSortError};
use camino::Utf8Path;
async fn plugin(host: &Host) -> aftercast::PluginId {
    let id = host.create_plugin(PluginConfig::builder().path(Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll"))
        .type_name("Aftercast.PluginFixture.ColumnsPlugin").build()).await.expect("column fixture");
    host.initialize_plugin(id).await.expect("initialize");
    id
}
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
        .application_executable(
            camino::Utf8PathBuf::try_from(application).expect("UTF8 executable"),
        )
        .build()
}
async fn receive(host: &Host, ui: &mut AppState, mode: Mode) -> Token {
    let (token, preferences) = ui.begin_table(mode).expect("admission");
    assert!(ui.begin_table(mode).is_none(), "duplicate callback request");
    assert!(!ui.edit_table(token.view, |_| panic!("busy edits must be rejected")));
    let result = read(host, &token, preferences, 0).await;
    assert!(ui.receive_table(token.clone(), result));
    token
}
fn editor(ui: &AppState) -> &Editor {
    ui.plugin_tables
        .editors
        .get(level_key(scope(ui).level()))
        .expect("editor")
}
fn page(ui: &AppState) -> &TablePage {
    ui.plugin_tables
        .frame
        .as_ref()
        .expect("frame")
        .page
        .as_ref()
        .expect("page")
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and actual synthetic Host; run mise run test:gui-state"]
async fn plugin_table_ui_contract_applies_saves_and_invalidates_real_column_frames() {
    let temporary = tempfile::tempdir().expect("isolated controller");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8 root");
    let config = config(root);
    let host = Host::start(config.clone()).await.expect("host");
    let id = plugin(&host).await;
    let input = root.join("source.log");
    std::fs::write(&input, "0|100\n0|300\n1|200\n").expect("synthetic source");
    host.import_log(&input).await.expect("import");
    let encounter = host
        .encounters(0, 16)
        .await
        .expect("graph")
        .items
        .first()
        .expect("encounter")
        .id;
    let mut ui = AppState::new(root.to_owned());
    ui.file = Some(ConfigFile::load(root).expect("config"));
    ui.phase = Phase::Ready;
    ui.revision.session = 1;
    ui.selected = Some(encounter);
    receive(&host, &mut ui, Mode::Refresh).await;
    assert!(
        dynamic(&ui),
        "the fixed Analysis table is replaced only by a current registered frame"
    );
    assert_eq!(editor(&ui).applied.visible, ["fixture"]);
    let cells = &page(&ui).rows.first().expect("row").cells;
    let cell = cells.first().expect("cell");
    assert_eq!(cell.foreground_argb, Some(-65536));
    assert!(render::cell_style(cell).contains("rgba(255, 0, 0, 1.0000)"));
    hidden_and_dirty_refresh(&host, &mut ui).await;
    let dirty = draft_and_save(&host, &mut ui, root).await;
    lifecycle(&host, &mut ui, id, &dirty).await;
    errors_and_navigation(&host, &mut ui).await;
    session_and_restart(host, config, &mut ui, root, &input).await;
}
async fn hidden_and_dirty_refresh(host: &Host, ui: &mut AppState) {
    assert!(ui.edit_table(view(ui), |draft| controls::toggle(draft, "probe")));
    let dirty = editor(ui).draft.selected();
    receive(host, ui, Mode::Refresh).await;
    assert_eq!(
        editor(ui).draft.selected(),
        dirty,
        "refresh preserves unapplied edits"
    );
    assert_eq!(
        page(ui).columns.len(),
        1,
        "refresh still uses the applied layout"
    );
    assert!(ui.edit_table(view(ui), |draft| draft.visible.clear()));
    receive(host, ui, Mode::Apply).await;
    assert!(
        dynamic(ui),
        "hidden columns retain the editor instead of switching to typed fallback"
    );
    assert!(
        ui.plugin_tables
            .frame
            .as_ref()
            .expect("frame")
            .page
            .is_none()
    );
    assert!(ui.edit_table(view(ui), |draft| controls::toggle(draft, "fixture")));
    receive(host, ui, Mode::Apply).await;
}
async fn draft_and_save(host: &Host, ui: &mut AppState, root: &Utf8Path) -> Preferences {
    let current = view(ui);
    assert!(ui.edit_table(current, |draft| {
        controls::toggle(draft, "probe");
        for _ in 0..3 {
            controls::move_key(draft, "probe", true);
        }
        draft.primary = Some("fixture".into());
        draft.descending = true;
    }));
    let mut draft = editor(ui).draft.clone();
    draft.visible = draft.selected();
    assert_eq!(draft.selected(), ["probe", "fixture"]);
    assert_eq!(
        page(ui).columns.first().expect("unapplied frame").key,
        "fixture"
    );
    assert!(
        ui.begin_table(Mode::Saved).is_none(),
        "saved layout must not discard a draft"
    );
    assert!(
        ui.begin_table_save(current).is_none(),
        "only applied layouts are saved"
    );
    let (token, preferences) = ui.begin_table(Mode::Apply).expect("apply");
    assert!(!ui.edit_table(current, |_| panic!("old render event")));
    ui.screen = "settings";
    let result = read(host, &token, preferences, 0).await;
    assert!(ui.receive_table(token, result));
    assert_eq!(ui.screen, "settings");
    assert_eq!(editor(ui).draft, draft);
    assert_eq!(editor(ui).applied, draft);
    assert_eq!(
        page(ui)
            .columns
            .iter()
            .map(|column| column.key.as_str())
            .collect::<Vec<_>>(),
        ["probe", "fixture"]
    );
    let frame = ui.plugin_tables.frame.as_ref().expect("frame");
    assert_eq!(
        frame
            .catalog
            .columns
            .iter()
            .find(|column| column.key == "fixture")
            .expect("fixture")
            .label,
        frame
            .catalog
            .columns
            .iter()
            .find(|column| column.key == "probe")
            .expect("probe")
            .label
    );
    save_failures(ui, root, &draft);
    draft
}
fn save_failures(ui: &mut AppState, root: &Utf8Path, draft: &Preferences) {
    let (failed, _, file) = ui.begin_table_save(view(ui)).expect("save");
    assert!(ui.begin_table_save(view(ui)).is_none(), "duplicate save");
    let obstacle = root.join("obstacle");
    std::fs::write(&obstacle, "owned file").expect("obstacle");
    assert!(ui.receive_table_save(failed.clone(), file.save(&obstacle), "saved".into()));
    assert!(ui.plugin_tables.save_error.is_some());
    assert!(!ui.saving);
    assert_eq!(&editor(ui).draft, draft);
    assert!(ui.settings.tables.is_empty());
    let (token, destination, file) = ui.begin_table_save(view(ui)).expect("retry save");
    assert!(!ui.receive_table_save(failed, Err(miette::miette!("old error")), "late".into()));
    assert!(ui.saving);
    assert!(ui.receive_table_save(token.clone(), file.save(&destination), "saved".into()));
    assert_eq!(ui.settings.tables.get("combatant"), Some(draft));
    assert_eq!(
        ConfigFile::load(root)
            .expect("reload")
            .settings
            .tables
            .get("combatant"),
        Some(draft)
    );
    assert!(!ui.receive_table_save(token, Err(miette::miette!("duplicate")), "duplicate".into()));
}
async fn lifecycle(host: &Host, ui: &mut AppState, id: aftercast::PluginId, saved: &Preferences) {
    host.deinitialize_plugin(id).await.expect("disable");
    ui.plugin_tables.invalidate();
    receive(host, ui, Mode::Refresh).await;
    assert!(!dynamic(ui));
    assert!(editor(ui).draft.order.is_empty());
    assert!(
        ui.plugin_tables
            .frame
            .as_ref()
            .expect("frame")
            .notices
            .iter()
            .any(|notice| notice.kind == "table-removed")
    );
    assert!(
        ui.begin_table_save(view(ui)).is_none(),
        "disabled plugin must not erase saved preferences"
    );
    assert_eq!(ui.settings.tables.get("combatant"), Some(saved));
    plugin(host).await;
    ui.plugin_tables.invalidate();
    receive(host, ui, Mode::Refresh).await;
    assert_eq!(
        editor(ui).draft.visible,
        ["fixture"],
        "re-registration appends plugin defaults to the working layout"
    );
    receive(host, ui, Mode::Saved).await;
    assert_eq!(
        &editor(ui).draft,
        saved,
        "explicit saved layout restores available registry keys"
    );
    let category = host
        .table_columns(TableLevel::Category)
        .await
        .expect("category metadata");
    let reconciliation = Preferences {
        order: vec!["fixture".into()],
        visible: vec!["fixture".into()],
        primary: Some("fixture".into()),
        ..Default::default()
    }
    .reconcile(&category)
    .expect("reset unsupported sort");
    assert!(reconciliation.preferences.primary.is_none());
    assert_eq!(reconciliation.reset_sorts, ["fixture"]);
    let catalog = ui
        .plugin_tables
        .frame
        .as_ref()
        .expect("frame")
        .catalog
        .clone();
    let mut many = catalog.clone();
    let template = many.columns.first().expect("column").clone();
    many.columns = (0..33)
        .map(|index| aftercast::TableColumn {
            key: format!("new{index}"),
            default_visible: true,
            ..template.clone()
        })
        .collect();
    let defaults = Preferences::default()
        .reconcile(&many)
        .expect("keep all defaults editable");
    assert_eq!(defaults.preferences.visible.len(), 33);
    defaults
        .preferences
        .validate()
        .expect_err("do not silently clip defaults");
}
async fn errors_and_navigation(host: &Host, ui: &mut AppState) {
    let current = view(ui);
    assert!(ui.edit_table(current, |draft| {
        controls::toggle(draft, "bad");
        controls::toggle(draft, "null");
        draft.primary = Some("bad".into());
    }));
    receive(host, ui, Mode::Apply).await;
    assert_eq!(page(ui).sort_error, Some(TableSortError::Callback));
    assert!(
        page(ui)
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .any(|cell| cell.error.is_some() && render::cell_style(cell).is_empty())
    );
    assert!(
        ui.begin_table_save(view(ui)).is_none(),
        "failed sort must be resolved before save"
    );
    assert!(ui.edit_table(view(ui), |draft| {
        draft.primary = None;
        draft.secondary = None;
        controls::toggle(draft, "mutate");
    }));
    let (token, preferences) = ui
        .begin_table(Mode::Apply)
        .expect("self-unregistering callback");
    assert!(ui.receive_table(token.clone(), read(host, &token, preferences, 0).await));
    assert!(ui.plugin_tables.error.is_some());
    assert!(ui.plugin_tables.frame.is_none());
    assert!(
        editor(ui).draft.visible.iter().any(|key| key == "mutate"),
        "failed query keeps the draft for explicit refresh"
    );
    receive(host, ui, Mode::Refresh).await;
    assert!(!editor(ui).draft.order.iter().any(|key| key == "mutate"));
    stale_and_navigation(host, ui).await;
}
async fn stale_and_navigation(host: &Host, ui: &mut AppState) {
    let old = ui
        .plugin_tables
        .frame
        .as_ref()
        .expect("frame")
        .token
        .clone();
    let (current, preferences) = ui.begin_table(Mode::Refresh).expect("new request");
    assert!(!ui.receive_table(old, Err("old error".into())));
    assert!(ui.plugin_tables.pending.is_some());
    ui.selected = None;
    let result = read(host, &current, preferences, 0).await;
    assert!(
        !ui.receive_table(current, result),
        "selection switch excludes the old table"
    );
    assert!(ui.phase == Phase::Ready);
    receive(host, ui, Mode::Refresh).await;
    assert!(matches!(scope(ui), TableScope::Encounters));
    drill_and_back(host, ui).await;
}
async fn drill_and_back(host: &Host, ui: &mut AppState) {
    let row = page(ui).rows.first().expect("encounter row").clone();
    let target = render::next(&row, TableLevel::Encounter).expect("drill down");
    assert!(ui.navigate_table(view(ui), Some(target)));
    receive(host, ui, Mode::Refresh).await;
    assert!(matches!(scope(ui), TableScope::Combatants(_)));
    let row = page(ui).rows.first().expect("actor row").clone();
    assert!(ui.navigate_table(view(ui), render::next(&row, TableLevel::Combatant)));
    receive(host, ui, Mode::Refresh).await;
    assert!(matches!(scope(ui), TableScope::Categories { .. }));
    assert!(ui.navigate_table(view(ui), None));
    receive(host, ui, Mode::Refresh).await;
    assert!(matches!(scope(ui), TableScope::Combatants(_)));
}
async fn session_and_restart(
    host: Host,
    config: HostConfig,
    ui: &mut AppState,
    root: &Utf8Path,
    input: &Utf8Path,
) {
    // A valid disk write may finish after Stop. It must update durable keys without replacing a new draft/frame.
    let applied = editor(ui).applied.clone();
    assert!(ui.edit_table(view(ui), |draft| *draft = applied));
    receive(&host, ui, Mode::Saved).await;
    let (save, destination, file) = ui.begin_table_save(view(ui)).expect("accepted local save");
    ui.revision.session += 1;
    ui.plugin_tables.reset_session();
    ui.selected = None;
    ui.phase = Phase::Idle;
    assert!(ui.receive_table_save(save, file.save(&destination), "late save".into()));
    assert!(ui.plugin_tables.frame.is_none());
    assert!(ui.plugin_tables.message.is_empty());
    ui.phase = Phase::Ready;
    let (old, preferences) = ui.begin_table(Mode::Refresh).expect("held reply");
    let result = read(&host, &old, preferences, 0).await;
    host.shutdown().await.expect("stop");
    ui.revision.session += 1;
    ui.plugin_tables.reset_session();
    ui.phase = Phase::Idle;
    assert!(!ui.receive_table(old, result));
    assert!(ui.plugin_tables.frame.is_none());
    let host = Host::start(config).await.expect("restart");
    plugin(&host).await;
    host.import_log(input).await.expect("reimport");
    ui.settings = ConfigFile::load(root).expect("persisted profile").settings;
    ui.file = Some(ConfigFile::load(root).expect("file"));
    ui.phase = Phase::Ready;
    ui.selected = Some(
        host.encounters(0, 16)
            .await
            .expect("new graph")
            .items
            .first()
            .expect("encounter")
            .id,
    );
    receive(&host, ui, Mode::Saved).await;
    assert_eq!(editor(ui).applied.selected(), ["probe", "fixture"]);
    assert_eq!(
        page(ui)
            .columns
            .iter()
            .map(|column| column.key.as_str())
            .collect::<Vec<_>>(),
        ["probe", "fixture"]
    );
    host.shutdown().await.expect("cleanup");
}

#[test]
fn refresh_clock_and_retired_requests_respect_session_and_visibility() {
    use std::time::Duration;
    let mut clock = RefreshClock::default();
    let t = |seconds| Duration::from_secs(seconds);
    assert!(!clock.tick(1, 5, true, true, t(0)));
    assert!(!clock.tick(1, 5, true, true, t(4)));
    assert!(!clock.tick(1, 5, true, false, t(5)));
    assert!(clock.tick(1, 5, true, true, t(6)));
    assert!(!clock.tick(1, 5, true, true, t(6)));
    assert!(!clock.tick(1, 2, true, true, t(7)));
    assert!(clock.tick(1, 2, true, true, t(9)));
    assert!(!clock.tick(2, 2, true, true, t(10)));
    assert!(!clock.tick(2, 2, false, true, t(12)));
    assert!(!clock.tick(2, 0, true, true, t(20)));
}

#[test]
fn automatic_table_requests_are_retired_on_navigation() {
    let mut ui = AppState::new(camino::Utf8PathBuf::from("/synthetic"));
    ui.phase = Phase::Ready;
    let (token, _) = ui.begin_table(Mode::Auto).expect("request");
    ui.retire_table_view();
    assert!(ui.phase == Phase::Ready);
    assert!(!ui.receive_table(token, Err("late".into())));
    assert!(ui.plugin_tables.error.is_none());
    let (fallback, stamp) = ui.begin_auto_fallback().expect("fallback");
    assert!(ui.begin_auto_fallback().is_none());
    ui.retire_table_view();
    assert!(!ui.loading);
    let (new_fallback, new_stamp) = ui.begin_auto_fallback().expect("reopened fallback");
    assert!(!ui.receive_auto_fallback(fallback, stamp, 0, Err(aftercast::Error::Closed)));
    assert!(ui.loading);
    assert!(ui.receive_auto_fallback(
        new_fallback,
        new_stamp,
        0,
        Ok(aftercast::EncounterPage {
            total: 0,
            items: vec![]
        })
    ));
    let (fallback, stamp) = ui.begin_auto_fallback().expect("next fallback");
    ui.revision.selection += 1;
    assert!(!ui.receive_auto_fallback(fallback, stamp, 0, Err(aftercast::Error::Closed)));
    assert!(!ui.loading && ui.error.is_none());
    assert!(!ui.loading && ui.error.is_none());
}

#[test]
fn automatic_table_requests_are_retired_on_exit() {
    let mut ui = AppState::new(camino::Utf8PathBuf::from("/synthetic"));
    ui.phase = Phase::Ready;
    let (fallback, stamp) = ui.begin_auto_fallback().expect("closing fallback");
    ui.exit_requested = true;
    assert!(!ui.receive_auto_fallback(fallback, stamp, 0, Err(aftercast::Error::Closed)));
    assert!(!ui.loading && ui.error.is_none());
    ui.exit_requested = false;
    let (token, _) = ui.begin_table(Mode::Auto).expect("closing table");
    ui.exit_requested = true;
    assert!(!ui.receive_table(token, Err("late close".into())));
    assert!(ui.phase == Phase::Ready && ui.plugin_tables.error.is_none());
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and actual synthetic Host; run mise run test:gui-state"]
async fn global_reverse_changes_only_selected_main_table_comparator() {
    let temporary = tempfile::tempdir().expect("fixture");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    let host = Host::start(config(root)).await.expect("host");
    plugin(&host).await;
    let input = root.join("reverse.log");
    std::fs::write(&input, "0|600\n1|50\n").expect("synthetic");
    host.import_log(&input).await.expect("import");
    let encounter = host.encounters(0, 16).await.expect("encounters").items[0].id;
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.revision.session = 1;
    ui.selected = Some(encounter);
    receive(&host, &mut ui, Mode::Refresh).await;
    assert_eq!(page(&ui).rows[0].combatant.as_deref(), Some("SOURCE"));
    ui.settings.ui.main_table_reverse = false;
    receive(&host, &mut ui, Mode::Refresh).await;
    assert_eq!(page(&ui).rows[0].combatant.as_deref(), Some("TARGET"));
    ui.settings.ui.main_table_reverse = true;
    receive(&host, &mut ui, Mode::Refresh).await;
    assert!(ui.edit_table(view(&ui), |draft| draft.primary = Some("fixture".into())));
    receive(&host, &mut ui, Mode::Apply).await;
    let descending = page(&ui)
        .rows
        .iter()
        .map(|row| row.combatant.clone())
        .collect::<Vec<_>>();
    ui.settings.ui.main_table_reverse = false;
    receive(&host, &mut ui, Mode::Refresh).await;
    let ascending = page(&ui)
        .rows
        .iter()
        .map(|row| row.combatant.clone())
        .collect::<Vec<_>>();
    assert_eq!(descending.len(), 2);
    assert_ne!(descending, ascending);
    assert_eq!(descending[0].as_deref(), Some("SOURCE"));
    assert_eq!(ascending[0].as_deref(), Some("TARGET"));
    let (old, preferences) = ui.begin_table(Mode::Refresh).expect("old request");
    ui.settings.ui.main_table_reverse = true;
    let old_result = read(&host, &old, preferences, 0).await;
    assert!(!ui.receive_table(old, old_result));
    assert!(ui.phase == Phase::Ready);
    assert!(!uses_global_reverse(TableLevel::Encounter));
    assert!(!uses_global_reverse(TableLevel::Category));
    host.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and actual synthetic Host; run mise run test:gui-state"]
async fn zone_ui_contract_preserves_identity_paging_and_rejects_late_results() {
    let temporary = tempfile::tempdir().expect("fixture");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    let config = config(root);
    let host = Host::start(config.clone()).await.expect("host");
    let id = host.create_plugin(PluginConfig::builder().path(Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll"))
        .type_name("Aftercast.PluginFixture.ZoneFixturePlugin").build()).await.expect("plugin");
    host.initialize_plugin(id).await.expect("init");
    let rows = host.encounters(0, 16).await.expect("encounters").items;
    let (first, second) = (rows[0].id, rows[2].id);
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.revision.session = 1;
    ui.selected = Some(first);
    ui.offset = 128;
    assert!(ui.select_zone(Some(first)));
    assert!(ui.selected.is_none() && ui.offset == 0);
    assert!(matches!(scope(&ui), TableScope::Zone(id) if id == first));
    let (old, prefs) = ui.begin_table(Mode::Auto).expect("first zone");
    let old_result = read(&host, &old, prefs, 0).await;
    ui.retire_table_view();
    ui.selected = Some(second);
    assert!(ui.select_zone(Some(second)));
    assert!(!ui.receive_table(old, old_result));
    let (token, prefs) = ui.begin_table(Mode::Refresh).expect("second zone");
    let loaded = read(&host, &token, prefs, 1).await.expect("second page");
    assert_eq!(loaded.page.as_ref().expect("page").total, 2);
    assert_eq!(
        loaded.page.as_ref().expect("page").rows[0].encounter,
        rows[3].id
    );
    assert!(ui.receive_table(token, Ok(loaded)));
    let (fallback, stamp) = ui.begin_auto_fallback().expect("fallback");
    let page = host
        .zone_encounters(second, 0, 1)
        .await
        .expect("zone")
        .expect("anchor");
    assert_eq!(page.total, 2);
    ui.retire_table_view();
    assert!(!ui.receive_auto_fallback(fallback, stamp, 0, Ok(page)));
    assert!(ui.select_zone(None));
    assert!(matches!(scope(&ui), TableScope::Encounters));
    ui.selected = Some(first);
    assert!(ui.select_zone(Some(first)));
    ui.revision.session += 1;
    ui.plugin_tables.reset_session();
    assert!(matches!(scope(&ui), TableScope::Encounters));
    host.shutdown().await.expect("stop");
    let fresh = Host::start(config).await.expect("restart");
    fresh
        .zone_encounters(first, 0, 1)
        .await
        .expect_err("old session");
    fresh.shutdown().await.expect("shutdown");
}
