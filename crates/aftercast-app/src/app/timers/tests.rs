use super::*;
use aftercast::Host;
use camino::Utf8Path;
async fn refresh(host: &Host, ui: &mut AppState) {
    let (offset, page) = read_list(host, ui.timers.offset)
        .await
        .expect("definitions");
    ui.timers.page(offset, page);
}
async fn acknowledge(host: &Host, ui: &mut AppState, mutation: Mutation) {
    let session = ui.begin_timer_request(true).expect("admit mutation");
    assert!(
        ui.begin_timer_request(true).is_none(),
        "duplicate mutation rejected"
    );
    assert!(ui.receive_timer_ack(
        session,
        &mutation,
        mutation.execute(host).await,
        "acknowledged".into()
    ));
    assert!(ui.finish_timer_request(session));
    refresh(host, ui).await;
}
pub(super) async fn contract(host: &Host, ui: &mut AppState, root: &Utf8Path) {
    let fixture = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            aftercast::PluginConfig::builder()
                .path(fixture)
                .type_name("Aftercast.PluginFixture.TimerMutationPlugin")
                .build(),
        )
        .await
        .expect("synthetic timers");
    host.initialize_plugin(plugin)
        .await
        .expect("initialize timers");
    refresh(host, ui).await;
    ui.timers.new_draft();
    ui.timers.draft.definition.name = "Local timer".into();
    ui.timers.draft.definition.category = String::new();
    ui.timers.draft.duration = "invalid number".into();
    ui.timers.dirty = true;
    ui.timers
        .draft
        .compile()
        .expect_err("invalid input must preserve draft");
    assert_eq!(ui.timers.draft.duration, "invalid number");
    ui.timers.draft.duration = "45".into();
    ui.timers.draft.color = "bad color".into();
    ui.timers
        .draft
        .compile()
        .expect_err("invalid input must preserve draft");
    ui.timers.draft.color = "#AABBCCDD".into();
    ui.screen = "settings";
    refresh(host, ui).await;
    assert_eq!(ui.timers.draft.duration, "45");
    assert_eq!(ui.screen, "settings");
    let definition = ui.timers.draft.compile().expect("valid draft");
    acknowledge(
        host,
        ui,
        Mutation::Save(ui.timers.revision.expect("revision"), None, definition),
    )
    .await;
    assert_eq!(
        ui.timers.draft.definition.category, " General",
        "use normalized managed response"
    );
    assert!(!ui.timers.dirty);
    assert_eq!(
        ui.timers.draft.color, "#AABBCCDD",
        "preserve alpha and signed ARGB bits"
    );
    failures(host, ui, root).await;
    conflicts(host, ui, root).await;
    deletion_and_stale(host, ui).await;
}
async fn failures(host: &Host, ui: &mut AppState, root: &Utf8Path) {
    ui.timers.draft.definition.tooltip = "kept after disk failure".into();
    ui.timers.dirty = true;
    let draft = ui.timers.draft.clone();
    let key = ui.timers.key.clone();
    let mutation = Mutation::Save(
        ui.timers.revision.expect("revision"),
        key.clone(),
        draft.compile().expect("draft"),
    );
    let path = root.join("state/SpellTimers.xml");
    let backup = root.join("state/SpellTimers.saved");
    std::fs::rename(&path, &backup).expect("move synthetic state");
    std::fs::create_dir(&path).expect("own blocker");
    let session = ui.begin_timer_request(true).expect("admit save");
    let result = mutation.execute(host).await;
    std::fs::remove_dir(&path).expect("remove own blocker");
    std::fs::rename(&backup, &path).expect("restore own state");
    assert!(!ui.receive_timer_ack(session, &mutation, result, "not acknowledged".into()));
    assert_eq!(ui.timers.draft, draft);
    assert_eq!(ui.timers.key, key);
    assert!(ui.timers.dirty);
    let mutation = Mutation::Save(
        ui.timers.revision.expect("revision"),
        key,
        draft.compile().expect("draft"),
    );
    let session = ui.begin_timer_request(true).expect("retry explicit save");
    assert!(ui.receive_timer_ack(
        session,
        &mutation,
        mutation.execute(host).await,
        "saved before read failure".into()
    ));
    ui.fail(aftercast::Error::Busy);
    assert!(!ui.timers.dirty && ui.timers.key.is_some());
    assert_eq!(ui.message, "saved before read failure");
    assert!(ui.finish_timer_request(session));
    refresh(host, ui).await;
}
async fn conflicts(host: &Host, ui: &mut AppState, root: &Utf8Path) {
    ui.timers.draft.definition.tooltip = "local conflict draft".into();
    ui.timers.dirty = true;
    let draft = ui.timers.draft.clone();
    let old = ui.timers.revision.expect("revision");
    let log = root.join("timer-conflict.log");
    std::fs::write(&log, "0|1|edit-timer\n").expect("synthetic import");
    host.import_log(&log).await.expect("external mutation");
    let mutation = Mutation::Save(old, ui.timers.key.clone(), draft.compile().expect("draft"));
    let session = ui.begin_timer_request(true).expect("save stale draft");
    assert!(!ui.receive_timer_ack(
        session,
        &mutation,
        mutation.execute(host).await,
        "not saved".into()
    ));
    refresh(host, ui).await;
    assert_eq!(ui.timers.draft, draft);
    assert_eq!(ui.timers.revision, Some(old));
    let page = ui.timers.page.as_ref().expect("page").clone();
    assert!(
        !ui.timers.select(page.items[0].clone(), page.revision),
        "row selection cannot discard draft"
    );
    let key = ui.timers.key.clone().expect("editing");
    assert!(ui.timers.rebase(&key, page.revision));
    acknowledge(
        host,
        ui,
        Mutation::Save(
            page.revision,
            Some(key.clone()),
            draft.compile().expect("draft"),
        ),
    )
    .await;
    ui.timers.dirty = true;
    host.delete_timer_definition(ui.timers.revision.expect("revision"), &key)
        .await
        .expect("external deletion");
    refresh(host, ui).await;
    let revision = ui.timers.page.as_ref().expect("page").revision;
    assert!(!ui.timers.rebase(&key, revision));
    assert!(ui.timers.keep_as_new(revision));
    ui.timers.draft.definition.name = "Fixture timer".into();
    ui.timers.draft.definition.category = "Fixture".into();
    let collision = Mutation::Save(revision, None, ui.timers.draft.compile().expect("draft"));
    let session = ui.begin_timer_request(true).expect("create collision");
    assert!(!ui.receive_timer_ack(
        session,
        &collision,
        collision.execute(host).await,
        "not saved".into()
    ));
    assert!(ui.timers.dirty && ui.timers.key.is_none());
    ui.timers.draft.definition.name = "New copy".into();
    let definition = ui.timers.draft.compile().expect("renamed draft");
    acknowledge(host, ui, Mutation::Save(revision, None, definition)).await;
}
async fn deletion_and_stale(host: &Host, ui: &mut AppState) {
    let revision = ui.timers.revision.expect("revision");
    let key = ui.timers.key.clone().expect("copy");
    let mutation = Mutation::Delete(revision, key.clone());
    ui.timers.deleting = Some(("fixture|fixture timer".into(), revision));
    acknowledge(host, ui, mutation).await;
    assert!(
        ui.timers
            .page
            .as_ref()
            .expect("page")
            .items
            .iter()
            .all(|item| item.key != key)
    );
    assert!(
        ui.timers
            .page
            .as_ref()
            .expect("page")
            .items
            .iter()
            .any(|item| item.definition.name == "Fixture timer")
    );
    pages(host, ui).await;
    let mutation = Mutation::Save(
        ui.timers.revision.expect("revision"),
        None,
        TimerDefinition {
            name: "Late".into(),
            ..Default::default()
        },
    );
    let old = ui.begin_timer_request(true).expect("old request");
    let response = mutation.execute(host).await;
    ui.revision.session += 1;
    ui.timers = Default::default();
    ui.phase = Phase::Stopping;
    assert!(!ui.receive_timer_ack(old, &mutation, response, "late reply".into()));
    assert!(ui.timers.key.is_none() && ui.phase == Phase::Stopping);
    ui.phase = Phase::Ready;
    let current = ui.begin_timer_request(true).expect("new worker request");
    assert!(!ui.finish_timer_request(old));
    assert!(ui.timers.busy && ui.phase == Phase::UpdatingTimers);
    assert!(ui.finish_timer_request(current));
}

async fn pages(host: &Host, ui: &mut AppState) {
    let mut revision = host.timer_definitions(0, 16).await.expect("page").revision;
    for index in 0..17 {
        let definition = TimerDefinition {
            name: format!("Page {index:02}"),
            category: "Pages".into(),
            ..Default::default()
        };
        revision = host
            .save_timer_definition(revision, None, &definition)
            .await
            .expect("synthetic paged definitions")
            .revision;
    }
    let (offset, page) = read_list(host, PAGE).await.expect("second page");
    assert_eq!(offset, PAGE);
    let last = page.items.clone();
    ui.timers.page(offset, page.clone());
    assert!(ui.timers.select(last[0].clone(), page.revision));
    ui.timers.draft.definition.tooltip = "kept across pages".into();
    ui.timers.dirty = true;
    let draft = ui.timers.draft.clone();
    let (offset, page) = read_list(host, 0).await.expect("previous page");
    ui.timers.page(offset, page);
    assert_eq!(ui.timers.draft, draft);
    for item in last {
        revision = host
            .delete_timer_definition(revision, &item.key)
            .await
            .expect("empty old last page");
    }
    let (offset, page) = read_list(host, PAGE)
        .await
        .expect("fallback after last page removed");
    assert_eq!(offset, 0);
    assert_eq!(page.retained_frames, 1);
    ui.timers.page(offset, page);
    ui.timers.new_draft();
}
