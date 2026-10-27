use super::*;
use aftercast::{Host, HostConfig, PluginConfig, extract_bridge};
use camino::Utf8Path;

fn definition(category: &str, regex: &str) -> TriggerDefinition {
    TriggerDefinition {
        category: category.into(),
        regex: regex.into(),
        ..Default::default()
    }
}
async fn refresh(host: &Host, ui: &mut AppState) {
    let (offset, page) = read_list(host, ui.triggers.offset)
        .await
        .expect("list refresh");
    ui.triggers.page(offset, page);
}
async fn acknowledge(host: &Host, ui: &mut AppState, mutation: Mutation) {
    let session = ui.begin_trigger_request(true).expect("admit mutation");
    assert!(
        ui.begin_trigger_request(true).is_none(),
        "duplicate click must not admit another request"
    );
    let result = mutation.execute(host).await;
    assert!(ui.receive_trigger_ack(session, &mutation, result, "saved".into()));
    assert!(ui.finish_trigger_request(session));
    refresh(host, ui).await;
}
async fn drafts(host: &Host, ui: &mut AppState) {
    ui.triggers.draft = definition("Local", "local");
    ui.triggers.dirty = true;
    let old = ui.triggers.revision.expect("initial revision");
    host.save_trigger(old, None, &definition("Remote", "remote"))
        .await
        .expect("external edit");
    refresh(host, ui).await;
    assert_ne!(
        ui.triggers.revision,
        Some(old),
        "new draft safely rebases after refresh"
    );
    assert_eq!(ui.triggers.draft.regex, "local");
    assert!(ui.triggers.dirty);
    let revision = ui.triggers.revision.expect("rebased revision");
    acknowledge(
        host,
        ui,
        Mutation::Save(revision, None, ui.triggers.draft.clone()),
    )
    .await;
    existing_draft_conflicts(host, ui).await;
}
async fn existing_draft_conflicts(host: &Host, ui: &mut AppState) {
    let item = ui
        .triggers
        .page
        .as_ref()
        .expect("page")
        .items
        .iter()
        .find(|item| item.definition.category == "Local")
        .expect("saved local")
        .clone();
    let revision = ui.triggers.page.as_ref().expect("page").revision;
    assert!(ui.triggers.select(item.clone(), revision));
    ui.triggers.draft.regex = "edited-local".into();
    ui.triggers.dirty = true;
    let draft = ui.triggers.draft.clone();
    assert!(
        !ui.triggers.select(item.clone(), revision),
        "selection must not discard a dirty draft"
    );
    ui.screen = "settings";
    ui.screen = "triggers";
    assert_eq!(
        ui.triggers.draft, draft,
        "navigation preserves the app-owned draft"
    );
    let mut remote = item.definition.clone();
    remote.sound_data = "remote change".into();
    host.save_trigger(revision, Some(&item.key), &remote)
        .await
        .expect("concurrent saved edit");
    refresh(host, ui).await;
    assert_eq!(
        ui.triggers.revision,
        Some(revision),
        "editing existing items must not silently rebase"
    );
    let reviewed = ui.triggers.page.as_ref().expect("reviewed page").revision;
    assert!(!ui.triggers.reviewed_rebase("wrong key", reviewed));
    assert!(ui.triggers.reviewed_rebase(&item.key, reviewed));
    assert_eq!(ui.triggers.draft, draft);
    acknowledge(
        host,
        ui,
        Mutation::Save(reviewed, Some(item.key), draft.clone()),
    )
    .await;
    ui.triggers.dirty = true;
    let key = ui.triggers.key.clone().expect("editing key");
    let revision = ui.triggers.revision.expect("editing revision");
    host.delete_trigger(revision, &key)
        .await
        .expect("external deletion");
    refresh(host, ui).await;
    let latest = ui.triggers.page.as_ref().expect("latest page").revision;
    assert!(!ui.triggers.reviewed_rebase(&key, latest));
    assert!(ui.triggers.keep_as_new(latest));
    assert_eq!(ui.triggers.draft, draft);
    assert!(ui.triggers.key.is_none());
    acknowledge(host, ui, Mutation::Save(latest, None, draft)).await;
}
fn pending_requests(ui: &mut AppState) {
    ui.triggers.dirty = true;
    let draft = ui.triggers.draft.clone();
    let session = ui.begin_trigger_request(true).expect("request");
    assert!(ui.begin_trigger_request(false).is_none());
    assert!(!ui.receive_trigger_ack(
        session,
        &Mutation::Clear,
        Err(aftercast::Error::Busy),
        "not committed".into()
    ));
    assert_eq!(ui.triggers.draft, draft);
    assert!(ui.triggers.dirty);
    assert!(ui.phase == Phase::Ready);
    assert!(ui.error.is_some());
    let session = ui.begin_trigger_request(true).expect("old request");
    ui.revision.session += 1;
    ui.triggers = Default::default();
    ui.phase = Phase::Ready;
    let new_session = ui.begin_trigger_request(true).expect("new worker request");
    assert!(!ui.receive_trigger_ack(session, &Mutation::Clear, Ok(None), "late old reply".into()));
    assert!(!ui.finish_trigger_request(session));
    assert!(ui.triggers.busy && ui.phase == Phase::UpdatingTriggers);
    assert!(ui.receive_trigger_ack(
        new_session,
        &Mutation::Clear,
        Ok(None),
        "acknowledged".into()
    ));
    ui.fail(aftercast::Error::Busy); // A later read can fail without changing the mutation acknowledgement.
    assert_eq!(ui.message, "acknowledged");
    assert!(ui.error.is_some());
    assert!(ui.finish_trigger_request(new_session));
}
async fn deletion_and_pages(host: &Host, ui: &mut AppState, root: &Utf8Path) {
    refresh(host, ui).await;
    let page = ui.triggers.page.as_ref().expect("page").clone();
    let original = page
        .items
        .iter()
        .find(|item| item.definition.category == "Local")
        .expect("local")
        .key
        .clone();
    let other = page
        .items
        .iter()
        .find(|item| item.definition.category == "Remote")
        .expect("remote")
        .key
        .clone();
    ui.triggers.deleting = Some((other.clone(), page.revision));
    acknowledge(host, ui, Mutation::Delete(page.revision, original.clone())).await;
    let items = &ui.triggers.page.as_ref().expect("page").items;
    assert!(items.iter().any(|item| item.key == other));
    assert!(!items.iter().any(|item| item.key == original));
    let assembly = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let parser = host
        .create_plugin(
            PluginConfig::builder()
                .path(assembly)
                .type_name("Aftercast.PluginFixture.ImportPlugin")
                .build(),
        )
        .await
        .expect("synthetic parser");
    host.initialize_plugin(parser).await.expect("parser ready");
    let revision = ui.triggers.page.as_ref().expect("page").revision;
    let revision = host
        .save_trigger(revision, None, &definition("Pages", r"^\d+\|\d+$"))
        .await
        .expect("result trigger");
    host.save_trigger_options(revision, false, true)
        .await
        .expect("import result processing");
    let path = root.join("results.log");
    std::fs::write(
        &path,
        (0..40).map(|n| format!("{n}|1\n")).collect::<String>(),
    )
    .expect("synthetic log");
    host.import_log(&path).await.expect("parse results");
    let first = host
        .trigger_results(None, PAGE)
        .await
        .expect("first results");
    let cursor = first.next_cursor;
    let next = host
        .trigger_results(Some(cursor), PAGE)
        .await
        .expect("next results");
    assert_eq!(first.items.len(), 16);
    assert_eq!(next.items.len(), 16);
    ui.triggers
        .accept_results(ResultMove::First, None, first.clone());
    ui.triggers
        .accept_results(ResultMove::Next, Some(cursor), next);
    let previous = ui.triggers.previous.last().copied().expect("return cursor");
    assert!(previous.is_none());
    let back = host
        .trigger_results(previous, PAGE)
        .await
        .expect("back to first page");
    ui.triggers
        .accept_results(ResultMove::Previous, previous, back);
    assert_eq!(
        ui.triggers.results.as_ref().expect("returned page").items,
        first.items
    );
    assert!(ui.triggers.previous.is_empty());
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:gui-state"]
async fn trigger_ui_contract_preserves_drafts_and_session_bound_work() {
    let temporary = tempfile::tempdir().expect("isolated UI contract data");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF-8 temp path");
    let bridge = root.join("bridge");
    extract_bridge(&bridge).expect("bridge payload");
    let executable = std::env::current_exe().expect("test binary");
    let application = executable
        .parent()
        .and_then(std::path::Path::parent)
        .expect("target directory")
        .join("examples")
        .join(format!("test-application{}", std::env::consts::EXE_SUFFIX));
    let host = Host::start(
        HostConfig::builder()
            .dotnet_root(std::env::var("DOTNET_ROOT").expect(".NET 10"))
            .bridge_directory(bridge)
            .data_directory(root.join("state"))
            .application_executable(
                camino::Utf8PathBuf::try_from(application).expect("UTF-8 app path"),
            )
            .build(),
    )
    .await
    .expect("host");
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.revision.session = 1;
    refresh(&host, &mut ui).await;
    drafts(&host, &mut ui).await;
    pending_requests(&mut ui);
    deletion_and_pages(&host, &mut ui, root).await;
    super::super::notifications::test_contract(&host, &mut ui).await;
    super::super::timers::test_contract(&host, &mut ui, root).await;
    host.shutdown().await.expect("owned worker cleanup");
}
