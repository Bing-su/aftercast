use super::*;
fn begin(f: &mut Fixture, current: bool) -> (Token, Request) {
    f.ui.report_odbc_begin(f.ui.report_token(), current)
        .expect("begin")
        .expect("admit")
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn odbc_ui_contract_synthetic_previews_cancel_and_stale_session() {
    let mut f = Fixture::new_with("OdbcPreviewPlugin", "0|100\n1|200\n").await;
    for current in [false, true] {
        let (token, request) = begin(&mut f, current);
        assert!(
            f.ui.report_odbc_begin(token, current)
                .expect("duplicate")
                .is_none()
        );
        let result = perform(&f.host.handle(), &request).await;
        assert!(f.ui.report_receive(token, result, ""));
        let preview = f.ui.report_preview().expect("preview");
        let Content::Odbc(text) = preview.report.as_ref() else {
            panic!("ODBC preview")
        };
        assert!(
            text.contains("CREATE TABLE") && text.contains("VALUES (?") && text.contains("enc")
        );
        assert_eq!(text.contains("DELETE FROM \"current_table\";"), current);
    }
    let token = f.ui.report_token();
    assert!(f.ui.report_odbc_term(token, "All".into()));
    let (token, request) = begin(&mut f, false);
    assert!(f.ui.report_cancel(token));
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert!(f.ui.report_preview().is_none());
    let (token, request) = begin(&mut f, false);
    assert!(matches!(&request, Request::Odbc(_, false, term, _) if term == "All"));
    assert!(!f.ui.report_odbc_term(token, "changed".into()));
    f.ui.screen = "other";
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    f.ui.screen = "analysis";
    f.ui.phase = Phase::Ready;
    let (token, request) = begin(&mut f, true);
    f.ui.reports.stop();
    f.ui.revision.session += 1;
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert!(f.ui.report_preview().is_none());
    f.host.shutdown().await.expect("shutdown");
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn odbc_ui_contract_failure_preserves_term() {
    let mut f = Fixture::new_with("OdbcPreviewPlugin", "0|666\n").await;
    assert!(f.ui.report_odbc_term(f.ui.report_token(), "All".into()));
    let (token, request) = begin(&mut f, false);
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.error.is_some());
    assert_eq!(f.ui.reports.odbc.all_term, "All");
    assert!(f.ui.report_preview().is_none());
    f.host.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn odbc_batch_ui_owns_rules_selection_and_cancel() {
    let mut f = Fixture::new_with("OdbcPreviewPlugin", "0|100\n1|200\n").await;
    let path = f.root.join("second-synthetic.log");
    std::fs::write(&path, "0|50\n1|60\n").expect("synthetic");
    f.host.import_log(path).await.expect("second import");
    f.ui.encounters = f.host.encounters(0, 16).await.expect("encounters").items;
    let ids: Vec<_> = f.ui.encounters.iter().map(|row| row.id).collect();
    assert_eq!(ids.len(), 2);
    for id in &ids {
        assert!(
            f.ui.report_html_toggle(f.ui.report_token(), *id)
                .expect("select")
        );
    }
    assert!(f.ui.report_odbc_rule_edit(f.ui.report_token(), "find", "encounter_table".into()));
    assert!(f.ui.report_odbc_rule_edit(
        f.ui.report_token(),
        "replacement",
        "synthetic_table".into()
    ));
    assert!(
        f.ui.report_odbc_rule_add(f.ui.report_token())
            .expect("rule")
    );
    let (token, request) =
        f.ui.report_odbc_batch_begin(f.ui.report_token())
            .expect("batch")
            .expect("request");
    assert!(
        f.ui.report_odbc_batch_begin(token)
            .expect("duplicate")
            .is_none()
    );
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    let preview = f.ui.report_preview().expect("batch preview");
    assert!(preview.report.text().contains("synthetic_table"));
    assert!(preview.report.text().contains("VALUES (?"));
    assert!(f.ui.report_odbc_rule_edit(f.ui.report_token(), "find", r"\?".into()));
    assert!(f.ui.report_odbc_rule_edit(f.ui.report_token(), "replacement", "1".into()));
    assert!(
        f.ui.report_odbc_rule_add(f.ui.report_token())
            .expect("unsupported rule")
    );
    let (failed_token, failed_request) =
        f.ui.report_odbc_batch_begin(f.ui.report_token())
            .expect("batch")
            .expect("request");
    let failed = perform(&f.host.handle(), &failed_request).await;
    assert!(f.ui.report_receive(failed_token, failed, ""));
    assert!(f.ui.reports.error.is_some());
    assert_eq!(f.ui.reports.odbc.rules.len(), 2, "failed draft retained");
    assert!(f.ui.report_odbc_rule_remove(f.ui.report_token(), 1));
    let (token, request) =
        f.ui.report_odbc_batch_begin(f.ui.report_token())
            .expect("batch")
            .expect("request");
    assert!(f.ui.report_cancel(token));
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    let (token, request) =
        f.ui.report_odbc_batch_begin(f.ui.report_token())
            .expect("batch")
            .expect("request");
    f.ui.reports.stop();
    f.ui.revision.session += 1;
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    f.host.shutdown().await.expect("shutdown");
}
