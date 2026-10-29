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
