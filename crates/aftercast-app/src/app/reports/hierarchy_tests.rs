use super::*;
use aftercast::HierarchyXmlDepth as Depth;
fn begin(f: &mut Fixture) -> (Token, Request) {
    f.ui.report_hierarchy_begin(f.ui.report_token())
        .expect("begin")
        .expect("admit")
}
async fn preview(f: &mut Fixture) {
    let (token, request) = begin(f);
    let reply = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, reply, ""));
    assert!(f.ui.report_preview().is_some(), "{:?}", f.ui.reports.error);
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_reports_ui_contract_depth_snapshots_and_exact_new_file_save() {
    let mut f = Fixture::new_with("HierarchyXmlPlugin", "0|100\n1|200\n").await;
    for (index, depth) in [Depth::MainTable, Depth::DamageTypeTables, Depth::AllTables]
        .into_iter()
        .enumerate()
    {
        let old = f.ui.report_token();
        assert!(f.ui.report_hierarchy_depth(old, depth));
        assert!(!f.ui.report_hierarchy_depth(old, Depth::MainTable));
        let (token, request) = begin(&mut f);
        assert!(
            f.ui.report_hierarchy_begin(token)
                .expect("duplicate")
                .is_none()
        );
        assert!(!f.ui.report_hierarchy_depth(token, Depth::MainTable));
        let reply = perform(&f.host.handle(), &request).await;
        assert!(f.ui.report_receive(token, reply, ""));
        let content = f.ui.report_preview().expect("preview").report.clone();
        let Content::Hierarchy(report) = content.as_ref() else {
            panic!("hierarchy")
        };
        assert_eq!(report.selection.depth(), depth);
        assert_eq!(
            report.selection.encounter(),
            f.ui.selected.expect("selected")
        );
        assert!(report.content.contains("<EncounterTable Name="));
        assert_eq!(report.content.contains("<CombatantTable Name="), index >= 1);
        assert_eq!(
            report.content.contains("<AttackTypeTable Name="),
            index == 2
        );
        assert!(report.content.contains("한글"));
        let (token, request) = f.save(&format!("hierarchy-{index}.xml"));
        let reply = perform(&f.host.handle(), &request).await;
        assert!(f.ui.report_receive(token, reply, "saved"));
        assert_eq!(
            std::fs::read(f.root.join(format!("hierarchy-{index}.xml"))).expect("bytes"),
            report.content.as_bytes()
        );
        let (token, request) = f.save(&format!("hierarchy-{index}.xml"));
        let reply = perform(&f.host.handle(), &request).await;
        assert!(f.ui.report_receive(token, reply, ""));
        assert!(f.ui.reports.error.is_some());
        assert_eq!(
            std::fs::read_to_string(f.root.join(format!("hierarchy-{index}.xml")))
                .expect("unchanged"),
            report.content
        );
    }
    let (token, request) = f.save("missing/report.xml");
    let reply = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, reply, ""));
    assert!(f.ui.reports.error.is_some());
    assert!(f.ui.report_preview().is_some());
    f.host.shutdown().await.expect("shutdown");
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_reports_ui_contract_cancel_navigation_selection_and_stop_retire_results() {
    let mut f = Fixture::new_with("HierarchyXmlPlugin", "0|100\n").await;
    assert!(f.ui.report_hierarchy_depth(f.ui.report_token(), Depth::AllTables));
    for mode in 0..4 {
        let (token, request) = begin(&mut f);
        match mode {
            0 => {
                assert!(f.ui.report_cancel(token));
            }
            1 => {
                f.ui.reports.leave();
                f.ui.screen = "history";
            }
            2 => {
                f.ui.revision.selection += 1;
                f.ui.selected = None;
            }
            _ => {
                f.ui.reports.stop();
                f.ui.revision.session += 1;
                f.ui.phase = Phase::Idle;
            }
        }
        let reply = perform(&f.host.handle(), &request).await;
        assert!(!f.ui.report_receive(token, reply, ""));
        assert!(f.ui.report_preview().is_none());
        assert_eq!(f.ui.reports.hierarchy.depth, Depth::AllTables);
        f.ui.screen = "analysis";
        f.ui.phase = Phase::Ready;
        f.ui.selected = Some(f.ui.encounters.first().expect("encounter").id);
    }
    preview(&mut f).await;
    let (token, request) = f.save("canceled.xml");
    assert!(f.ui.report_cancel(token));
    let reply = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, reply, ""));
    assert!(!f.root.join("canceled.xml").exists());
    preview(&mut f).await;
    f.ui.report_edit(
        f.ui.report_token(),
        "path",
        f.root.join("obsolete.xml").to_string(),
    );
    f.ui.report_review_save(f.ui.report_token())
        .expect("review");
    let old = f.ui.report_token();
    assert!(f.ui.report_hierarchy_depth(old, Depth::MainTable));
    assert!(f.ui.report_confirm_save(old).is_none());
    assert!(f.ui.report_preview().is_none());
    f.host.shutdown().await.expect("shutdown");
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_reports_ui_contract_callback_timeout_and_output_errors_preserve_draft() {
    for (text, message) in [
        ("0|666\n".to_owned(), "synthetic hierarchy SQL failure"),
        ("0|444\n".to_owned(), "exceeded"),
        ((0..200).map(|n| format!("{n}|999\n")).collect(), "16 MiB"),
    ] {
        let mut f = Fixture::new_with("HierarchyXmlPlugin", &text).await;
        assert!(f.ui.report_hierarchy_depth(f.ui.report_token(), Depth::AllTables));
        let (token, request) = begin(&mut f);
        let reply = perform(&f.host.handle(), &request).await;
        assert!(f.ui.report_receive(token, reply, ""));
        let error = f.ui.reports.error.as_deref().expect("explicit error");
        assert!(
            error.to_lowercase().contains(&message.to_lowercase()),
            "{error}"
        );
        assert_eq!(f.ui.reports.hierarchy.depth, Depth::AllTables);
        assert!(f.ui.report_preview().is_none());
        f.ui.report_review_save(f.ui.report_token())
            .expect_err("failed generation cannot be saved");
        // Recovery is an explicit new request with a deliberately shallower depth.
        assert!(f.ui.report_hierarchy_depth(f.ui.report_token(), Depth::MainTable));
        if !text.contains("666") {
            preview(&mut f).await;
        }
        f.host.shutdown().await.expect("shutdown");
    }
}
