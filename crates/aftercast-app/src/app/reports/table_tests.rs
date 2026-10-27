use super::*;
use aftercast::{TableReportFormat, TableScope};
fn edit(ui: &mut AppState, key: &str, value: &str) {
    assert!(
        ui.report_table_edit(ui.report_token(), key, value.into()),
        "{key}: {value}"
    );
}
async fn load(f: &mut Fixture) {
    let (token, request) =
        f.ui.report_table_begin(f.ui.report_token(), false)
            .expect("load")
            .expect("admit");
    assert!(
        f.ui.report_table_begin(token, false)
            .expect("duplicate")
            .is_none()
    );
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
}
fn render(ui: &mut AppState) -> (Token, Request) {
    ui.report_table_begin(ui.report_token(), true)
        .expect("generate")
        .expect("admit")
}
async fn preview(f: &mut Fixture) {
    let (token, request) = render(&mut f.ui);
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.report_preview().is_some(), "{:?}", f.ui.reports.error);
}
async fn verify_scope_files(f: &mut Fixture) {
    for (level, root) in [
        ("combatant", "EncounterTable"),
        ("category", "CombatantTable"),
        ("attack", "DamageTypeTable"),
        ("action", "AttackTypeTable"),
    ] {
        edit(&mut f.ui, "level", level);
        edit(&mut f.ui, "actor", "Source");
        edit(&mut f.ui, "category", "Damage");
        edit(&mut f.ui, "attack", "All");
        load(f).await;
        assert!(f.ui.report_table_catalog().is_some());
        preview(f).await;
        let output =
            f.ui.report_preview()
                .expect("preview")
                .report
                .text()
                .to_owned();
        assert!(output.contains(&format!("<{root} Name=")));
        assert!(output.contains("&#xD;&#xA;"));
        assert!(output.contains("<sql_name>"));
        let (token, request) = f.save(&format!("{level}.xml"));
        assert!(f.ui.report_confirm_save(token).is_none());
        let result = perform(&f.host.handle(), &request).await;
        assert!(f.ui.report_receive(token, result, "saved"));
        assert_eq!(
            std::fs::read(f.root.join(format!("{level}.xml"))).expect("file"),
            output.as_bytes()
        );
    }
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn table_reports_ui_contract_four_scopes_xml_html_snapshot_and_new_files() {
    let mut f = Fixture::new_with("TableReportPlugin", "0|100\n1|300\n2|200\n").await;
    edit(&mut f.ui, "name", "한글日本😀\r\n<caption>&\"");
    verify_scope_files(&mut f).await;
    edit(&mut f.ui, "row", "0");
    edit(&mut f.ui, "row", "2");
    preview(&mut f).await;
    let snapshot = f.ui.report_preview().expect("preview").report.as_ref();
    let Content::Table(report, options) = snapshot else {
        panic!("table report")
    };
    assert_eq!(report.rows, 2);
    assert_eq!(options.selected_rows, [0, 2]);
    assert!(report.content.contains("100</sql_name>"));
    assert!(!report.content.contains("300</sql_name>"));
    assert!(report.content.contains("200</sql_name>"));
    edit(&mut f.ui, "format", "html");
    f.ui.report_table_begin(f.ui.report_token(), true)
        .expect_err("explicit HTML columns required");
    edit(&mut f.ui, "column", "fixture");
    preview(&mut f).await;
    let Content::Table(html, options) = f.ui.report_preview().expect("HTML").report.as_ref() else {
        panic!("table")
    };
    assert_eq!(html.format, TableReportFormat::StaticHtml);
    assert_eq!(html.rows, 3);
    assert!(options.selected_rows.is_empty());
    assert_eq!(options.columns.as_ref().expect("keys"), &["fixture"]);
    assert!(html.content.contains("&lt;script&gt;"));
    assert!(!html.content.contains("<script>"));
    let generated = html.content.clone();
    let (token, request) = f.save("report.html");
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, "saved"));
    assert_eq!(
        std::fs::read_to_string(f.root.join("report.html")).expect("HTML file"),
        generated
    );
    let (token, request) = f.save("report.html");
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.error.is_some());
    assert_eq!(
        std::fs::read_to_string(f.root.join("report.html")).expect("no overwrite"),
        generated
    );
    edit(&mut f.ui, "column", "fixture");
    assert!(f.ui.reports.table.columns.is_empty());
    load(&mut f).await;
    assert!(
        f.ui.reports.table.columns.is_empty(),
        "explicit empty column draft retained"
    );
    edit(&mut f.ui, "format", "xml");
    assert!(
        f.ui.reports.table.selected.is_empty(),
        "recapture invalidated original row indices"
    );
    edit(&mut f.ui, "level", "combatant");
    load(&mut f).await;
    edit(&mut f.ui, "primary", "fixture");
    edit(&mut f.ui, "descending", "true");
    preview(&mut f).await;
    let sorted = f.ui.report_preview().expect("sorted").report.text();
    assert!(
        sorted.find("Target</sql_name>").expect("target")
            < sorted.find("Source</sql_name>").expect("source")
    );
    edit(&mut f.ui, "format", "html");
    edit(&mut f.ui, "column", "bad-sort");
    edit(&mut f.ui, "column", "fixture");
    edit(&mut f.ui, "up", "fixture");
    preview(&mut f).await;
    let Content::Table(_, options) =
        f.ui.report_preview()
            .expect("ordered columns")
            .report
            .as_ref()
    else {
        panic!("table")
    };
    assert_eq!(
        options.columns.as_ref().expect("keys"),
        &["fixture", "bad-sort"]
    );
    f.host.shutdown().await.expect("stop");
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn table_reports_ui_contract_cancel_navigation_stale_stop_and_failed_save() {
    let mut f = Fixture::new_with("TableReportPlugin", "0|100\n1|200\n").await;
    load(&mut f).await;
    edit(&mut f.ui, "name", "draft");
    let stale = f.ui.report_token();
    edit(&mut f.ui, "name", "kept");
    assert!(!f.ui.report_table_edit(stale, "format", "html".into()));
    let (token, request) = render(&mut f.ui);
    assert!(!f.ui.report_table_edit(token, "name", "raced".into()));
    assert!(
        f.ui.report_table_begin(token, true)
            .expect("duplicate")
            .is_none()
    );
    assert!(f.ui.report_cancel(token));
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert_eq!(f.ui.reports.table.name, "kept");
    assert!(f.ui.report_preview().is_none());
    preview(&mut f).await;
    let (token, request) = f.save("canceled.xml");
    assert!(f.ui.report_cancel(token));
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert!(!f.root.join("canceled.xml").exists());
    preview(&mut f).await;
    let (token, request) = f.save("missing/report.xml");
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.error.is_some());
    assert!(f.ui.report_preview().is_some());
    let token = f.ui.report_token();
    f.ui.report_edit(token, "path", f.root.join("old.xml").to_string());
    f.ui.report_review_save(f.ui.report_token())
        .expect("review");
    let old = f.ui.report_token();
    assert!(f.ui.report_cancel_review(old));
    f.ui.report_review_save(f.ui.report_token())
        .expect("new review");
    assert!(f.ui.report_confirm_save(old).is_none());
    f.ui.report_cancel_review(f.ui.report_token());
    let (token, request) = render(&mut f.ui);
    f.ui.reports.leave();
    f.ui.screen = "history";
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert!(!f.ui.report_table_edit(token, "name", "hidden".into()));
    f.ui.screen = "analysis";
    let (token, request) = render(&mut f.ui);
    f.ui.revision.selection += 1;
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert!(f.ui.phase == Phase::Ready);
    assert!(f.ui.report_preview().is_none());
    preview(&mut f).await;
    let (token, request) = f.save("stopped.xml");
    f.ui.reports.stop();
    f.ui.revision.session += 1;
    f.ui.phase = Phase::Idle;
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert!(!f.root.join("stopped.xml").exists());
    assert!(f.ui.reports.table_catalog.is_none());
    f.host.shutdown().await.expect("stop");
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn table_reports_ui_contract_scope_copy_missing_errors_and_bounded_source_preview() {
    let mut f = Fixture::new_with("TableReportPlugin", "0|666\n").await;
    f.ui.combatant = Some("Source".into());
    f.ui.category = Some(("Damage".into(), true));
    assert!(f.ui.report_table_copy(f.ui.report_token()));
    assert!(
        matches!(f.ui.report_table_scope(),Some(TableScope::Attacks{combatant,category,..}) if combatant=="Source"&&category=="Damage")
    );
    edit(&mut f.ui, "level", "combatant");
    load(&mut f).await;
    edit(&mut f.ui, "name", "preserved on failure");
    let (token, request) = render(&mut f.ui);
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(
        f.ui.reports
            .error
            .as_deref()
            .is_some_and(|error| error.contains("synthetic SQL failure"))
    );
    assert_eq!(f.ui.reports.table.name, "preserved on failure");
    assert!(f.ui.report_preview().is_none());
    f.ui.report_review_save(f.ui.report_token())
        .expect_err("no failed report save");
    edit(&mut f.ui, "format", "html");
    edit(&mut f.ui, "column", "fixture");
    preview(&mut f).await;
    let large = "x".repeat(65535) + "😀tail";
    if let Content::Table(report, _) = Arc::make_mut(
        &mut f
            .ui
            .reports
            .preview
            .as_mut()
            .expect("source preview")
            .report,
    ) {
        report.content = large.clone();
    }
    let content = &f.ui.report_preview().expect("bounded preview").report;
    assert_eq!(content.preview_text().len(), 65535);
    assert!(
        content
            .text()
            .is_char_boundary(content.preview_text().len())
    );
    let (token, request) = f.save("full-source.html");
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, "saved"));
    assert_eq!(
        std::fs::read_to_string(f.root.join("full-source.html")).expect("complete source"),
        large
    );
    edit(&mut f.ui, "primary", "");
    edit(&mut f.ui, "secondary", "fixture");
    f.ui.report_table_begin(f.ui.report_token(), true)
        .expect_err("secondary needs primary");
    assert_eq!(f.ui.reports.table.secondary, "fixture");
    edit(&mut f.ui, "secondary", "");
    edit(&mut f.ui, "level", "category");
    edit(&mut f.ui, "actor", "absent");
    load(&mut f).await;
    assert_eq!(f.ui.reports.error.as_deref(), Some("report-unavailable"));
    assert_eq!(f.ui.reports.table.actor, "absent");
    assert!(f.ui.report_table_catalog().is_none());
    edit(&mut f.ui, "actor", "Source");
    load(&mut f).await;
    assert!(f.ui.report_table_catalog().is_some());
    assert!(
        !f.ui
            .report_table_edit(f.ui.report_token(), "page", usize::MAX.to_string())
    );
    f.host.shutdown().await.expect("stop");
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn table_reports_ui_contract_text_table_switch_invalidates_old_save_confirmation() {
    let mut f = Fixture::new_with("TableReportPlugin", "0|100\n").await;
    load(&mut f).await;
    preview(&mut f).await;
    assert!(f.ui.report_edit(
        f.ui.report_token(),
        "path",
        f.root.join("old.xml").to_string()
    ));
    f.ui.report_review_save(f.ui.report_token())
        .expect("table review");
    let old = f.ui.report_token();
    let (token, request) =
        f.ui.report_begin(f.ui.report_token(), false)
            .expect("text catalog")
            .expect("admit");
    assert!(f.ui.report_confirm_save(old).is_none());
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    f.ui.report_edit(f.ui.report_token(), "player", "ordinary".into());
    f.ui.report_edit(f.ui.report_token(), "only", "false".into());
    f.preview().await;
    assert!(matches!(
        f.ui.report_preview().expect("text").report.as_ref(),
        Content::Text(..)
    ));
    assert!(
        f.ui.report_preview()
            .expect("text")
            .report
            .text()
            .contains("ordinary")
    );
    preview(&mut f).await;
    assert!(matches!(
        f.ui.report_preview().expect("table").report.as_ref(),
        Content::Table(..)
    ));
    assert_eq!(f.ui.reports.draft.player_format, "ordinary");
    assert!(!f.root.join("old.xml").exists());
    f.host.shutdown().await.expect("stop");
}
