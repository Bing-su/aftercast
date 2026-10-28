use super::*;
use aftercast::TableLevel;
async fn load(f: &mut Fixture) {
    let (token, request) =
        f.ui.report_html_begin(f.ui.report_token(), false)
            .expect("load")
            .expect("admit");
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert_eq!(f.ui.reports.html.catalogs.len(), 4);
}
fn toggle(f: &mut Fixture, id: EncounterId) {
    assert!(
        f.ui.report_html_toggle(f.ui.report_token(), id)
            .expect("toggle")
    );
}
fn edit(f: &mut Fixture, level: TableLevel, field: &str, value: &str) {
    assert!(
        f.ui.report_html_edit(f.ui.report_token(), level, field, value.into()),
        "{level:?}/{field}/{value}"
    );
}
fn depth(f: &mut Fixture, level: TableLevel) {
    assert!(f.ui.report_html_depth(f.ui.report_token(), level));
}
fn begin(f: &mut Fixture) -> (Token, Request) {
    f.ui.report_html_begin(f.ui.report_token(), true)
        .expect("begin")
        .expect("admit")
}
async fn preview(f: &mut Fixture) {
    let (token, request) = begin(f);
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.report_preview().is_some(), "{:?}", f.ui.reports.error);
}
async fn import(f: &mut Fixture, text: &str) {
    let path = f.root.join("next synthetic.log");
    std::fs::write(&path, text).expect("synthetic input");
    f.host.import_log(&path).await.expect("import");
    f.ui.encounters = f.host.encounters(0, 128).await.expect("tree").items;
}
fn expanded_columns(f: &mut Fixture, count: usize) {
    edit(f, TableLevel::Action, "column", "registered-b");
    for index in 0..count {
        edit(
            f,
            TableLevel::Action,
            "column",
            &format!("expanded-{index}"),
        );
    }
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_html_reports_ui_contract_multi_depth_snapshot_and_new_file_bytes() {
    let mut f = Fixture::new_with("HierarchyHtmlPlugin", "0|100\n1|200\n").await;
    import(&mut f, "100|100\n").await;
    let ids: Vec<_> = f.ui.encounters.iter().map(|row| row.id).collect();
    assert_eq!(ids.len(), 2);
    load(&mut f).await;
    for id in ids.iter().rev() {
        toggle(&mut f, *id);
    }
    for level in super::super::html::LEVELS {
        edit(&mut f, level, "column", "registered-a");
    }
    edit(&mut f, TableLevel::Combatant, "primary", "registered-a");
    edit(&mut f, TableLevel::Combatant, "descending", "true");
    assert!(!f.ui.report_html_edit(
        f.ui.report_token(),
        TableLevel::Category,
        "primary",
        "registered-a".into()
    ));
    for (index, level) in super::super::html::LEVELS.into_iter().enumerate() {
        depth(&mut f, level);
        let (token, request) = begin(&mut f);
        assert!(
            f.ui.report_html_begin(token, true)
                .expect("duplicate")
                .is_none()
        );
        assert!(!f.ui.report_html_depth(token, TableLevel::Combatant));
        assert!(
            !f.ui
                .report_html_toggle(token, ids[0])
                .expect("pending selection")
        );
        let result = perform(&f.host.handle(), &request).await;
        assert!(f.ui.report_receive(token, result, ""));
        let content = f.ui.report_preview().expect("preview").report.clone();
        let Content::Html(report, settings, selected, _) = content.as_ref() else {
            panic!("HTML")
        };
        assert_eq!(report.selection.last_level(), level);
        assert_eq!(report.selection.encounters(), ids);
        assert_eq!(selected.iter().map(|row| row.id).collect::<Vec<_>>(), ids);
        assert_eq!(settings.len(), index + 1);
        assert_eq!(
            settings.first().expect("main").columns,
            ["registered-b", "registered-a"]
        );
        assert!(
            settings
                .first()
                .expect("main")
                .sort
                .as_ref()
                .expect("sort")
                .descending
        );
        assert!(report.content.contains("&lt;script&gt;") && !report.content.contains("<script"));
        assert!(report.content.contains("default-src 'none'"));
        let (token, request) = f.save(&format!("hierarchy-{index}.html"));
        let result = perform(&f.host.handle(), &request).await;
        assert!(f.ui.report_receive(token, result, "saved"));
        assert_eq!(
            std::fs::read(f.root.join(format!("hierarchy-{index}.html"))).expect("bytes"),
            report.content.as_bytes()
        );
        let (token, request) = f.save(&format!("hierarchy-{index}.html"));
        let result = perform(&f.host.handle(), &request).await;
        assert!(f.ui.report_receive(token, result, ""));
        assert!(f.ui.reports.error.is_some());
        assert_eq!(
            std::fs::read_to_string(f.root.join(format!("hierarchy-{index}.html")))
                .expect("unchanged"),
            report.content
        );
    }
    let old = f.ui.report_token();
    toggle(&mut f, ids[0]);
    assert!(f.ui.report_preview().is_none());
    assert!(!f.ui.report_html_depth(old, TableLevel::Combatant));
    f.host.shutdown().await.expect("shutdown");
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_html_reports_ui_contract_cancel_stale_navigation_stop_and_save_failure() {
    let mut f = Fixture::new_with("HierarchyHtmlPlugin", "0|100\n").await;
    load(&mut f).await;
    let id = f.ui.encounters.first().expect("candidate").id;
    toggle(&mut f, id);
    depth(&mut f, TableLevel::Action);
    for mode in 0..3 {
        let (token, request) = begin(&mut f);
        match mode {
            0 => {
                assert!(f.ui.report_cancel(token));
            }
            1 => {
                f.ui.reports.leave();
                f.ui.screen = "history";
            }
            _ => {
                f.ui.revision.selection += 1;
                f.ui.selected = None;
            }
        }
        assert!(
            f.ui.report_html_begin(f.ui.report_token(), true)
                .expect("still pending")
                .is_none()
        );
        let result = perform(&f.host.handle(), &request).await;
        assert!(!f.ui.report_receive(token, result, ""));
        assert!(f.ui.report_preview().is_none());
        assert_eq!(f.ui.reports.html.selected.len(), 1);
        assert_eq!(f.ui.reports.html.last, TableLevel::Action);
        f.ui.screen = "analysis";
        f.ui.selected = Some(id);
    }
    preview(&mut f).await;
    let (token, request) = f.save("canceled.html");
    assert!(f.ui.report_cancel(token));
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert!(!f.root.join("canceled.html").exists());
    preview(&mut f).await;
    let (token, request) = f.save("missing/report.html");
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.error.is_some());
    assert!(f.ui.report_preview().is_some());
    f.ui.report_edit(
        f.ui.report_token(),
        "path",
        f.root.join("old.html").to_string(),
    );
    f.ui.report_review_save(f.ui.report_token())
        .expect("review");
    let old = f.ui.report_token();
    edit(&mut f, TableLevel::Combatant, "primary", "registered-a");
    assert!(f.ui.report_confirm_save(old).is_none());
    assert!(f.ui.report_preview().is_none());
    let (token, request) = begin(&mut f);
    let columns =
        f.ui.reports
            .html
            .levels
            .first()
            .expect("main")
            .columns
            .clone();
    f.ui.reports.stop();
    f.ui.revision.session += 1;
    f.ui.phase = Phase::Idle;
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.html.selected.is_empty());
    assert!(f.ui.reports.html.catalogs.is_empty());
    assert_eq!(
        f.ui.reports.html.levels.first().expect("preserved").columns,
        columns
    );
    assert_eq!(f.ui.reports.html.last, TableLevel::Action);
    f.ui.phase = Phase::Ready;
    f.ui.report_html_begin(f.ui.report_token(), true)
        .expect_err("new session requires explicit selection");
    load(&mut f).await;
    toggle(&mut f, id);
    preview(&mut f).await;
    let (token, request) =
        f.ui.report_html_begin(f.ui.report_token(), false)
            .expect("reload")
            .expect("admit");
    f.ui.reports.stop();
    f.ui.revision.session += 1;
    f.ui.phase = Phase::Idle;
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.html.catalogs.is_empty());
    f.host.shutdown().await.expect("shutdown");
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_html_reports_ui_contract_bounds_drafts_and_actual_registry_changes() {
    let mut f = Fixture::new_with("HierarchyHtmlPlugin", "0|6400\n").await;
    import(&mut f, "100|100\n").await;
    assert_eq!(f.ui.encounters.len(), 65);
    load(&mut f).await;
    let ids: Vec<_> = f.ui.encounters.iter().map(|row| row.id).collect();
    f.ui.report_html_begin(f.ui.report_token(), true)
        .expect_err("no automatic selection");
    for id in ids.iter().take(64) {
        toggle(&mut f, *id);
    }
    assert_eq!(f.ui.reports.html.selected.len(), 64);
    f.ui.report_html_toggle(f.ui.report_token(), *ids.last().expect("65th"))
        .expect_err("65th encounter rejected");
    assert_eq!(f.ui.reports.html.selected.len(), 64);
    assert!(f.ui.report_html_page(f.ui.report_token(), 4));
    assert!(!f.ui.report_html_page(f.ui.report_token(), usize::MAX));
    let (token, request) = begin(&mut f);
    let Request::HtmlRender(selected, _, _, _, _) = &request else {
        panic!("admitted snapshot")
    };
    assert_eq!(selected.len(), 64);
    assert!(f.ui.report_cancel(token));
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    for id in ids.iter().skip(1).take(63) {
        toggle(&mut f, *id);
    }
    // Backend list pages replace the candidates; the explicit selected set survives them.
    f.ui.encounters = f
        .host
        .encounters(16, 16)
        .await
        .expect("next backend page")
        .items;
    let next = f.ui.encounters.first().expect("new page candidate").id;
    toggle(&mut f, next);
    assert_eq!(
        f.ui.reports
            .html
            .selected
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        [ids[0], next]
    );
    preview(&mut f).await;
    let Content::Html(report, _, selected, _) =
        f.ui.report_preview()
            .expect("cross-page result")
            .report
            .as_ref()
    else {
        panic!("HTML")
    };
    assert_eq!(report.selection.encounters(), [ids[0], next]);
    assert_eq!(
        selected.iter().map(|row| row.id).collect::<Vec<_>>(),
        [ids[0], next]
    );
    toggle(&mut f, ids[0]);
    assert_eq!(
        f.ui.reports.html.selected.len(),
        1,
        "remove selected ID absent from current candidates"
    );
    toggle(&mut f, next);
    f.ui.encounters = f
        .host
        .encounters(0, 128)
        .await
        .expect("first candidates")
        .items;
    toggle(&mut f, ids[0]);
    registry_draft_recovery(&mut f).await;
    f.host.shutdown().await.expect("shutdown");
}
async fn registry_draft_recovery(f: &mut Fixture) {
    edit(f, TableLevel::Combatant, "column", "registered-b");
    load(f).await;
    assert!(
        f.ui.reports
            .html
            .levels
            .first()
            .expect("main")
            .columns
            .is_empty()
    );
    f.ui.report_html_begin(f.ui.report_token(), true)
        .expect_err("explicit empty remains empty");
    edit(f, TableLevel::Combatant, "column", "registered-b");
    edit(f, TableLevel::Combatant, "secondary", "registered-a");
    f.ui.report_html_begin(f.ui.report_token(), true)
        .expect_err("secondary needs primary");
    edit(f, TableLevel::Combatant, "secondary", "");
    import(f, "200|1111\n").await;
    let (token, request) = begin(f);
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.error.is_some());
    assert_eq!(
        f.ui.reports.html.levels.first().expect("draft").columns,
        ["registered-b"]
    );
    load(f).await;
    f.ui.report_html_begin(f.ui.report_token(), true)
        .expect_err("missing key visible and preserved");
    import(f, "300|1112\n").await;
    load(f).await;
    preview(f).await;
    assert!(
        f.ui.report_preview()
            .expect("re-registered key")
            .report
            .text()
            .contains("re-registered")
    );
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_html_reports_ui_contract_errors_timeout_and_output_preserve_request_drafts() {
    for (text, message) in [
        ("0|666\n".to_owned(), "synthetic hierarchy display failure"),
        ("0|444\n".to_owned(), "exceeded"),
        ((0..8).map(|n| format!("{n}|999\n")).collect(), "16 MiB"),
    ] {
        let mut f = Fixture::new_with("HierarchyHtmlPlugin", &text).await;
        load(&mut f).await;
        let id = f.ui.encounters.first().expect("encounter").id;
        toggle(&mut f, id);
        depth(&mut f, TableLevel::Action);
        edit(&mut f, TableLevel::Combatant, "column", "registered-a");
        if text.contains("999") {
            expanded_columns(&mut f, 32);
        }
        let (token, request) = begin(&mut f);
        let result = perform(&f.host.handle(), &request).await;
        assert!(f.ui.report_receive(token, result, ""));
        let error = f.ui.reports.error.as_deref().expect("explicit error");
        assert!(error.contains(message), "{error}");
        assert!(f.ui.report_preview().is_none());
        assert_eq!(f.ui.reports.html.last, TableLevel::Action);
        assert_eq!(f.ui.reports.html.selected.len(), 1);
        f.ui.report_review_save(f.ui.report_token())
            .expect_err("no partial save");
        if text.contains("666") {
            edit(&mut f, TableLevel::Combatant, "column", "registered-a");
        } else if text.contains("999") {
            depth(&mut f, TableLevel::Combatant);
            assert_eq!(
                f.ui.reports
                    .html
                    .levels
                    .last()
                    .expect("hidden action draft")
                    .columns
                    .len(),
                32
            );
        }
        preview(&mut f).await;
        f.host.shutdown().await.expect("shutdown");
    }
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_html_reports_ui_contract_bounded_text_preview_saves_full_owned_source() {
    let mut f = Fixture::new_with("HierarchyHtmlPlugin", "0|999\n1|999\n").await;
    load(&mut f).await;
    let id = f.ui.encounters.first().expect("encounter").id;
    toggle(&mut f, id);
    depth(&mut f, TableLevel::Action);
    edit(&mut f, TableLevel::Action, "column", "registered-b");
    for n in 0..4 {
        edit(
            &mut f,
            TableLevel::Action,
            "column",
            &format!("expanded-{n}"),
        );
    }
    edit(&mut f, TableLevel::Action, "up", "expanded-3");
    edit(&mut f, TableLevel::Action, "down", "expanded-3");
    preview(&mut f).await;
    let content = f.ui.report_preview().expect("source").report.clone();
    assert!(content.text().len() > 65536);
    assert!(content.preview_text().len() <= 65536);
    assert!(
        content
            .text()
            .is_char_boundary(content.preview_text().len())
    );
    let (token, request) = f.save("full-source.html");
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, "saved"));
    assert_eq!(
        std::fs::read(f.root.join("full-source.html")).expect("full bytes"),
        content.text().as_bytes()
    );
    f.host.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_html_ui_contract_graph_snapshot_and_no_overwrite() {
    let mut f = Fixture::new_with("HierarchyHtmlPlugin", "0|100\n3|200\n").await;
    load(&mut f).await;
    let id = f.ui.encounters[0].id;
    toggle(&mut f, id);
    let token = f.ui.report_token();
    assert!(f.ui.report_html_graph_edit(token, "toggle", "3".into()));
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "distribution", "Resist".into()));
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "action_sort", "registered-a".into()));
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "window", "1".into()));
    let (token, request) = begin(&mut f);
    assert!(!f.ui.report_html_graph_edit(token, "clear", String::new()));
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    let content = f.ui.report_preview().expect("preview").report.clone();
    let Content::Html(report, settings, _, graphs) = content.as_ref() else {
        panic!("HTML")
    };
    assert_eq!(settings.len(), 4);
    assert_eq!(graphs.as_ref().unwrap().levels, [TableLevel::Action]);
    assert!(
        report.content.contains("<svg")
            && report.content.contains("<rect")
            && !report.content.contains("<polyline")
    );
    let (token, request) = f.save("graph-report.html");
    assert!(f.ui.report_receive(token, perform(&f.host.handle(), &request).await, "saved"));
    assert_eq!(
        std::fs::read(f.root.join("graph-report.html")).unwrap(),
        report.content.as_bytes()
    );
    let (token, request) = f.save("graph-report.html");
    assert!(f.ui.report_receive(token, perform(&f.host.handle(), &request).await, ""));
    assert!(f.ui.reports.error.is_some());
    f.host.shutdown().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_html_ui_contract_graph_explicit_choices_are_owned_and_stale_safe() {
    let mut f = Fixture::new_with("HierarchyHtmlPlugin", "0|100\n3|200\n").await;
    load(&mut f).await;
    let id = f.ui.encounters[0].id;
    toggle(&mut f, id);
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "toggle", "0".into()));
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "simple", "false".into()));
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "toggle", "1".into()));
    let (token, request) =
        f.ui.report_html_graph_candidates_begin(f.ui.report_token())
            .unwrap()
            .unwrap();
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    let candidates = f.ui.reports.html.graph_candidates.clone().unwrap();
    let actor = candidates.actors.first().unwrap().clone();
    assert!(candidates.categories.iter().any(|key| key == "Damage"));
    assert!(f.ui.report_html_graph_scope(f.ui.report_token(), "actors_mode", "custom"));
    assert!(f.ui.report_html_graph_scope(f.ui.report_token(), "actors", &actor));
    assert!(f.ui.report_html_graph_scope(f.ui.report_token(), "categories_mode", "custom"));
    assert!(f.ui.report_html_graph_scope(f.ui.report_token(), "categories", "Damage"));
    let (token, request) = begin(&mut f);
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    let content = f.ui.report_preview().unwrap().report.clone();
    let Content::Html(report, _, _, graphs) = content.as_ref() else {
        panic!("HTML")
    };
    let graph = graphs.as_ref().unwrap();
    assert_eq!(graph.selected_combatants.as_ref().unwrap(), &[actor]);
    assert_eq!(graph.solo_categories.as_ref().unwrap(), &["Damage"]);
    assert!(report.content.contains("<polyline"));
    f.ui.reports.html.graph_actors = Some(vec!["Missing choice".into()]);
    assert!(matches!(
        f.ui.report_html_begin(f.ui.report_token(), true),
        Err("html-graph-missing-choices")
    ));
    assert!(f.ui.report_html_graph_scope(f.ui.report_token(), "actors", "Missing choice"));
    assert_eq!(f.ui.reports.html.graph_actors, Some(vec![]));
    f.ui.reports.html.graph_actors = Some(vec!["hidden stale actor".into()]);
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "toggle", "0".into()));
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "simple", "false".into()));
    let (token, request) = begin(&mut f);
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(
        f.ui.report_preview().is_some(),
        "inactive actor choice does not block solo graph"
    );
    toggle(&mut f, id);
    assert!(f.ui.reports.html.graph_candidates.is_none());
    assert!(f.ui.report_preview().is_none());
    f.host.shutdown().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_html_ui_contract_graph_choices_reject_changed_registry() {
    let mut f = Fixture::new_with("HierarchyHtmlPlugin", "0|100\n3|200\n").await;
    load(&mut f).await;
    let id = f.ui.encounters[0].id;
    toggle(&mut f, id);
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "toggle", "1".into()));
    let (token, request) =
        f.ui.report_html_graph_candidates_begin(f.ui.report_token())
            .unwrap()
            .unwrap();
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.report_html_graph_scope(f.ui.report_token(), "categories_mode", "custom"));
    assert!(f.ui.report_html_graph_scope(f.ui.report_token(), "categories", "Damage"));
    let (token, request) = begin(&mut f);
    let path = f.root.join("synthetic-registry-change.log");
    std::fs::write(&path, "100|1113\n").unwrap();
    f.host
        .import_log(&path)
        .await
        .expect("synthetic registry mutation");
    let result = perform(&f.host.handle(), &request).await;
    assert!(result.is_err(), "stale registry rejected");
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.report_preview().is_none());
    f.host.shutdown().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_html_ui_contract_graph_choices_reject_same_key_actor_replacement() {
    let mut f = Fixture::new_with("HierarchyHtmlPlugin", "0|100\n3|200\n").await;
    load(&mut f).await;
    let id = f.ui.encounters[0].id;
    toggle(&mut f, id);
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "toggle", "0".into()));
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "simple", "false".into()));
    let (token, request) =
        f.ui.report_html_graph_candidates_begin(f.ui.report_token())
            .unwrap()
            .unwrap();
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    let actor =
        f.ui.reports
            .html
            .graph_candidates
            .as_ref()
            .unwrap()
            .actors
            .first()
            .unwrap()
            .clone();
    assert!(f.ui.report_html_graph_scope(f.ui.report_token(), "actors_mode", "custom"));
    assert!(f.ui.report_html_graph_scope(f.ui.report_token(), "actors", &actor));
    let (token, request) = begin(&mut f);
    let path = f.root.join("synthetic-actor-replacement.log");
    std::fs::write(&path, "100|1114\n").unwrap();
    f.host
        .import_log(&path)
        .await
        .expect("synthetic replacement");
    let result = perform(&f.host.handle(), &request).await;
    assert!(result.is_err(), "same-key replacement rejected");
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.report_preview().is_none());
    f.host.shutdown().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_html_ui_contract_all_mode_retires_graph_choice_capture() {
    let mut f = Fixture::new_with("HierarchyHtmlPlugin", "0|100\n3|200\n").await;
    load(&mut f).await;
    let id = f.ui.encounters[0].id;
    toggle(&mut f, id);
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "toggle", "0".into()));
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "simple", "false".into()));
    let (token, request) =
        f.ui.report_html_graph_candidates_begin(f.ui.report_token())
            .unwrap()
            .unwrap();
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.html.graph_candidates.is_some());
    let (token, request) = begin(&mut f);
    assert!(f.ui.reports.html.graph_candidates.is_none());
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    let preview = f.ui.report_preview().unwrap().report.clone();
    let Content::Html(report, _, _, graph) = preview.as_ref() else {
        panic!("HTML")
    };
    assert!(graph.as_ref().unwrap().selected_combatants.is_none());
    assert!(
        report.content.contains("<polyline"),
        "all actors generate series"
    );
    let (token, request) =
        f.ui.report_html_graph_candidates_begin(f.ui.report_token())
            .unwrap()
            .unwrap();
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.report_html_graph_scope(f.ui.report_token(), "actors_mode", "custom"));
    let (token, request) = begin(&mut f);
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    let preview = f.ui.report_preview().unwrap().report.clone();
    let Content::Html(report, _, _, graph) = preview.as_ref() else {
        panic!("HTML")
    };
    assert_eq!(graph.as_ref().unwrap().selected_combatants, Some(vec![]));
    assert!(
        !report.content.contains("<polyline"),
        "explicit empty choice renders no actor series"
    );
    f.host.shutdown().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_html_ui_contract_graph_candidates_retire_table_selection() {
    let mut f = Fixture::new_with("HierarchyHtmlPlugin", "0|100\n3|200\n").await;
    load(&mut f).await;
    let id = f.ui.encounters[0].id;
    toggle(&mut f, id);
    let (token, request) =
        f.ui.report_table_begin(f.ui.report_token(), false)
            .unwrap()
            .unwrap();
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.report_table_catalog().is_some());
    assert!(f.ui.report_table_edit(f.ui.report_token(), "row", "0".into()));
    let (token, request) =
        f.ui.report_html_graph_candidates_begin(f.ui.report_token())
            .unwrap()
            .unwrap();
    assert!(f.ui.report_table_catalog().is_none());
    assert!(f.ui.reports.table.selected.is_empty());
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.html.graph_candidates.is_some());
    f.host.shutdown().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn hierarchy_html_ui_contract_simple_graph_snapshot_and_save() {
    let mut f = Fixture::new_with("HierarchyHtmlPlugin", "0|100\n3|200\n").await;
    load(&mut f).await;
    let id = f.ui.encounters[0].id;
    toggle(&mut f, id);
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "toggle", "0".into()));
    assert!(f.ui.report_html_graph_edit(
        f.ui.report_token(),
        "simple_column",
        "simple-numeric".into()
    ));
    assert!(f.ui.report_html_graph_edit(f.ui.report_token(), "window", "unused-invalid".into()));
    let (token, request) = begin(&mut f);
    assert!(
        !f.ui.report_html_graph_edit(token, "totals", "false".into()),
        "pending snapshot is immutable"
    );
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    let preview =
        f.ui.report_preview()
            .expect("Simple preview")
            .report
            .clone();
    let Content::Html(report, _, _, graph) = preview.as_ref() else {
        panic!("HTML")
    };
    assert!(
        graph.as_ref().unwrap().simple
            && report.content.contains("<rect")
            && report.content.contains("AVG:")
    );
    let (token, request) = f.save("simple-graph.html");
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, "saved"));
    assert_eq!(
        std::fs::read(f.root.join("simple-graph.html")).unwrap(),
        report.content.as_bytes()
    );
    let (token, request) = f.save("simple-graph.html");
    let result = perform(&f.host.handle(), &request).await;
    assert!(f.ui.report_receive(token, result, ""));
    assert!(f.ui.reports.error.is_some());
    let (token, request) = begin(&mut f);
    assert!(f.ui.report_cancel(token));
    let result = perform(&f.host.handle(), &request).await;
    assert!(!f.ui.report_receive(token, result, ""));
    f.host.shutdown().await.unwrap();
}
