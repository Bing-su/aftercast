mod common;
use aftercast::{Error, Host, PluginConfig, TriggerDefinition};
use camino::Utf8Path;
use common::{Deployment, deployment};

async fn parser(host: &Host) {
    let assembly = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = host
        .create_plugin(
            PluginConfig::builder()
                .path(assembly)
                .type_name("Aftercast.PluginFixture.ImportPlugin")
                .build(),
        )
        .await
        .expect("parser");
    host.initialize_plugin(id).await.expect("initialize parser");
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn trigger_edits_results_and_options_survive_the_correct_boundaries(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    parser(&host).await;
    let first = host.triggers(0, 16).await.expect("empty definitions");
    let definition = TriggerDefinition {
        category: "Import Zone".into(),
        regex: r"^0\|(?<amount>\d+)$".into(),
        sound_type: 3,
        sound_data: "${amount}".into(),
        active: true,
        restrict_to_category_zone: true,
        timer: true,
        timer_name: "Fixture results".into(),
        tabbed: true,
    };
    let revision = host
        .save_trigger(first.revision, None, &definition)
        .await
        .expect("create and persist");
    host.save_trigger(revision, None, &definition)
        .await
        .expect_err("duplicate rejected");
    let items = host.triggers(0, 16).await.expect("created definition");
    let key = items.items[0].key.clone();
    let mut invalid = definition.clone();
    invalid.regex = "[".into();
    host.save_trigger(revision, Some(&key), &invalid)
        .await
        .expect_err("invalid regex preserves definition");
    let enabled = host
        .save_trigger_options(revision, false, true)
        .await
        .expect("enable import results");
    host.save_trigger(revision, Some(&key), &definition)
        .await
        .expect_err("stale editor rejected");
    let log = deployment.directory().join("triggers.log");
    std::fs::write(&log, "0|100\n0|300\n").expect("synthetic log");
    assert_eq!(host.import_log(&log).await.expect("import").damage, 400);
    let page = host
        .trigger_results(None, 1)
        .await
        .expect("first captured result");
    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].is_import);
    assert_eq!(page.items[0].line, "0|100");
    assert!(
        page.items[0]
            .captures
            .iter()
            .any(|capture| capture.name == "amount" && capture.value == "100")
    );
    let next = host
        .trigger_results(Some(page.next_cursor), 16)
        .await
        .expect("next captured result");
    assert_eq!(next.items.len(), 1);
    assert_eq!(next.items[0].line, "0|300");
    assert_ne!(next.items[0].time_sorter, page.items[0].time_sorter);
    let mut edited = definition.clone();
    edited.regex = r"^0\|(?<amount>300)$".into();
    edited.category = "Changed".into();
    edited.restrict_to_category_zone = false;
    let revision = host
        .save_trigger(enabled, Some(&key), &edited)
        .await
        .expect("rename/recompile/persist");
    assert!(
        host.trigger_results(None, 16)
            .await
            .expect("old result tab removed")
            .items
            .is_empty()
    );
    host.import_log(&log).await.expect("updated matcher");
    let result = host
        .trigger_results(None, 16)
        .await
        .expect("only changed regex matches");
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].line, "0|300");
    host.clear_trigger_results()
        .await
        .expect("clear transient results");
    host.shutdown().await.expect("stop");
    let restarted = Host::start(deployment.config()).await.expect("restart");
    parser(&restarted).await;
    let saved = restarted
        .triggers(0, 16)
        .await
        .expect("restored definition");
    assert_eq!(saved.items.len(), 1);
    assert_eq!(saved.items[0].definition, edited);
    assert!(saved.process_imports);
    assert!(
        restarted
            .trigger_results(None, 16)
            .await
            .expect("new session has no results")
            .items
            .is_empty()
    );
    assert!(matches!(
        restarted.trigger_results(Some(page.next_cursor), 16).await,
        Err(Error::InvalidTriggerRequest(_))
    ));
    assert!(matches!(
        restarted
            .delete_trigger(revision, &saved.items[0].key)
            .await,
        Err(Error::InvalidTriggerRequest(_))
    ));
    restarted
        .delete_trigger(saved.revision, &saved.items[0].key)
        .await
        .expect("delete and persist");
    let revision = restarted
        .triggers(0, 16)
        .await
        .expect("empty after deletion")
        .revision;
    restarted
        .reload_triggers(revision)
        .await
        .expect("reload empty saved registry");
    assert_eq!(
        restarted.triggers(0, 16).await.expect("still empty").total,
        0
    );
    restarted.shutdown().await.expect("stop restart");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn archive_trigger_results_are_explicit_and_independent_of_raw_import(
    deployment: Deployment,
) {
    let host = Host::start(deployment.config()).await.expect("host");
    parser(&host).await;
    let page = host.triggers(0, 16).await.expect("triggers");
    let definition = TriggerDefinition {
        category: "fixture".into(),
        regex: r"^0\|(?<amount>\d+)$".into(),
        sound_type: 3,
        sound_data: "${amount}".into(),
        timer: true,
        timer_name: "fixture".into(),
        ..TriggerDefinition::default()
    };
    host.save_trigger(page.revision, None, &definition)
        .await
        .expect("register trigger");
    let file = deployment.directory().join("archive.xml");
    std::fs::write(&file, "<Encounters><Encounter Name=\"Synthetic\"><LogLine Type=\"1\" Time=\"2020-01-01T00:00:00\" Line=\"0|123\" /></Encounter></Encounters>").expect("fixture");
    host.import_act(&file).await.expect("default import");
    assert!(
        host.trigger_results(None, 16)
            .await
            .expect("results")
            .items
            .is_empty()
    );
    host.import_act_with_trigger_results(&file, true)
        .await
        .expect("explicit trigger matching");
    let results = host.trigger_results(None, 16).await.expect("results");
    assert_eq!(results.items.len(), 1);
    assert_eq!(results.items[0].line, "0|123");
    assert!(results.items[0].is_import);
    assert!(
        results.items[0]
            .captures
            .iter()
            .any(|capture| capture.name == "amount" && capture.value == "123")
    );
    host.clear_trigger_results().await.expect("clear");
    std::fs::write(&file, "<Encounters><Encounter Name=\"Invalid\"><LogLine Type=\"1\" Time=\"2020-01-01T00:00:00\" Line=\"0|123\" /><Invalid /></Encounter></Encounters>").expect("bad fixture");
    host.import_act_with_trigger_results(&file, true)
        .await
        .expect_err("invalid archive");
    assert!(
        host.trigger_results(None, 16)
            .await
            .expect("no partial result")
            .items
            .is_empty()
    );
    let before = host.encounters(0, 16).await.expect("encounters").total;
    let line = "<LogLine Type=\"1\" Time=\"2020-01-01T00:00:00\" Line=\"0|123\" />";
    let oversized = format!(
        "<Encounters><Encounter Name=\"budget\">{}</Encounter></Encounters>",
        line.repeat(10_001)
    );
    std::fs::write(&file, oversized).expect("bounded fixture");
    let error = host
        .import_act_with_trigger_results(&file, true)
        .await
        .expect_err("line budget");
    assert!(error.to_string().contains("10000"), "{error}");
    assert_eq!(
        host.encounters(0, 16)
            .await
            .expect("unchanged encounters")
            .total,
        before
    );
    assert!(
        host.trigger_results(None, 16)
            .await
            .expect("no staged results")
            .items
            .is_empty()
    );
    std::fs::write(
        &file,
        format!("<Encounters><Encounter Name=\"retry\">{line}</Encounter></Encounters>"),
    )
    .expect("retry fixture");
    host.import_act_with_trigger_results(&file, true)
        .await
        .expect("retry once");
    assert_eq!(
        host.encounters(0, 16).await.expect("retry count").total,
        before + 1
    );
    assert_eq!(
        host.trigger_results(None, 16)
            .await
            .expect("retry result")
            .items
            .len(),
        1
    );
    host.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn trigger_snippet_preview_is_read_only_and_owned(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    let xml = r#"<Trigger R="A&#92;s+&#35;(?&lt;n&gt;[0-9]+)" C="Fixture" SD="&lt;hello&gt;" ST="3" T="T" TN="Timer" Ta="T" />"#;
    let preview = host.preview_trigger_snippet(xml).await.expect("preview");
    assert_eq!(preview.definition.regex, r"A\s+#(?<n>[0-9]+)");
    assert_eq!(preview.definition.sound_data, "<hello>");
    assert!(!preview.exists);
    assert_eq!(host.triggers(0, 16).await.expect("no mutation").total, 0);
    host.preview_trigger_snippet("<Spell N='x' C='y'/>")
        .await
        .expect_err("unsupported spell");
    host.preview_trigger_snippet(
        "<!DOCTYPE Trigger [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><Trigger R='&x;' C='y'/>",
    )
    .await
    .expect_err("DTD prohibited");
    host.save_trigger(preview.revision, None, &preview.definition)
        .await
        .expect("explicit import");
    assert!(
        host.preview_trigger_snippet(xml)
            .await
            .expect("duplicate preview")
            .exists
    );
    host.save_trigger(preview.revision, None, &preview.definition)
        .await
        .expect_err("stale revision");
    host.shutdown().await.expect("stop");
    let restarted = Host::start(deployment.config()).await.expect("restart");
    assert_eq!(restarted.triggers(0, 16).await.expect("persisted").total, 1);
    restarted
        .save_trigger(preview.revision, None, &preview.definition)
        .await
        .expect_err("old session");
    restarted.shutdown().await.expect("stop restart");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn trigger_config_batch_is_atomic_and_session_owned(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    let xml = "<Config><CustomTriggers><Trigger Regex='one' Category='Fixture'/><Trigger Regex='two' Category='Fixture'/></CustomTriggers></Config>";
    let preview = host.preview_trigger_config(xml).await.expect("preview");
    assert_eq!(preview.items.len(), 2);
    assert!(preview.conflicts.is_empty());
    assert_eq!(host.triggers(0, 16).await.expect("no changes").total, 0);
    host.preview_trigger_config("<Config><CustomTriggers/><SpellTimers/></Config>")
        .await
        .expect_err("mixed rejected");
    let items = preview
        .items
        .iter()
        .map(|item| item.definition.clone())
        .collect::<Vec<_>>();
    host.save_trigger_batch(preview.revision, &items)
        .await
        .expect("atomic batch");
    host.save_trigger_batch(preview.revision, &items)
        .await
        .expect_err("stale");
    assert_eq!(
        host.preview_trigger_config(xml)
            .await
            .expect("conflict")
            .conflicts
            .len(),
        2
    );
    host.shutdown().await.expect("shutdown");
    let restarted = Host::start(deployment.config()).await.expect("restart");
    assert_eq!(restarted.triggers(0, 16).await.expect("persisted").total, 2);
    restarted
        .save_trigger_batch(preview.revision, &items)
        .await
        .expect_err("old session");
    restarted.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn mixed_config_owns_input_and_both_revisions(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    let mut xml = "<Config><CustomTriggers><Trigger Regex='mixed' Category='Fixture'/></CustomTriggers><SpellTimers><Spell Name='Mixed' Category='Fixture'/></SpellTimers></Config>".to_owned();
    let stale_timer = host.preview_mixed_config(&xml).await.expect("preview");
    assert_eq!(host.triggers(0, 16).await.unwrap().total, 0);
    assert_eq!(host.timer_definitions(0, 16).await.unwrap().total, 0);
    let timers = host.timer_definitions(0, 16).await.unwrap();
    let timer = aftercast::TimerDefinition {
        name: "Other".into(),
        category: "Fixture".into(),
        ..Default::default()
    };
    host.save_timer_definition(timers.revision, None, &timer)
        .await
        .unwrap();
    host.import_mixed_config(&stale_timer)
        .await
        .expect_err("stale timer revision");
    let stale_trigger = host.preview_mixed_config(&xml).await.unwrap();
    let page = host.triggers(0, 16).await.unwrap();
    let trigger = TriggerDefinition {
        category: "Fixture".into(),
        regex: "other".into(),
        ..Default::default()
    };
    host.save_trigger(page.revision, None, &trigger)
        .await
        .unwrap();
    host.import_mixed_config(&stale_trigger)
        .await
        .expect_err("stale trigger revision");
    let preview = host.preview_mixed_config(&xml).await.unwrap();
    xml.clear();
    host.import_mixed_config(&preview)
        .await
        .expect("owned snapshot");
    host.import_mixed_config(&preview)
        .await
        .expect_err("duplicate retry");
    assert_eq!(host.triggers(0, 16).await.unwrap().total, 2);
    assert_eq!(host.timer_definitions(0, 16).await.unwrap().total, 2);
    host.shutdown().await.unwrap();
    let restarted = Host::start(deployment.config()).await.unwrap();
    restarted
        .import_mixed_config(&preview)
        .await
        .expect_err("old session");
    assert_eq!(restarted.triggers(0, 16).await.unwrap().total, 2);
    assert_eq!(restarted.timer_definitions(0, 16).await.unwrap().total, 2);
    restarted.shutdown().await.unwrap();
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn mixed_publication_failure_retires_isolated_worker(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    let assembly = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = host
        .create_plugin(
            PluginConfig::builder()
                .path(assembly)
                .type_name("Aftercast.PluginFixture.MixedConfigFaultPlugin")
                .build(),
        )
        .await
        .unwrap();
    host.initialize_plugin(id)
        .await
        .expect("synthetic fault injected");
    host.triggers(0, 16)
        .await
        .expect_err("faulted worker rejects requests");
    if let Err(error) = host.shutdown().await {
        eprintln!("faulted worker shutdown: {error}");
    }
    let restarted = Host::start(deployment.config())
        .await
        .expect("fresh worker loads committed files");
    assert_eq!(restarted.triggers(0, 16).await.unwrap().total, 1);
    assert_eq!(restarted.timer_definitions(0, 16).await.unwrap().total, 1);
    restarted.shutdown().await.unwrap();
}
