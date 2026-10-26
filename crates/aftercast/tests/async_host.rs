mod common;

use std::time::Duration;

use aftercast::{Error, Host, HostHandle, PluginConfig};
use camino::Utf8Path;
use common::{Deployment, deployment};

#[test]
fn handles_are_send_and_sync() {
    fn check<T: Send + Sync>() {}
    check::<HostHandle>();
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn imports_cancel_restore_and_shutdown_without_caller_pumping(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("start");
    let handle = host.handle();
    audio_cleanup_can_query_the_host(&host).await;
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = handle
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.ImportPlugin")
                .build(),
        )
        .await
        .expect("fixture");
    handle.initialize_plugin(plugin).await.expect("initialize");
    let input = deployment.directory().join("long.log");
    std::fs::write(&input, "0|1\n".repeat(100_000)).expect("long source");
    let job = handle.start_import(&input).await.expect("admitted import");
    assert!(matches!(
        handle.start_import(&input).await,
        Err(Error::Busy)
    ));
    assert!(matches!(
        handle.deinitialize_plugin(plugin).await,
        Err(Error::Busy)
    ));
    // A long-running import must leave the owner available for diagnostics and progress publication.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while job.progress().records_dispatched == 0 {
        handle
            .diagnostics()
            .await
            .expect("owner responsive during import");
        assert!(std::time::Instant::now() < deadline, "no import progress");
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert!(job.progress().bytes_read <= job.progress().total_bytes);
    job.cancel();
    assert!(matches!(job.wait().await, Err(Error::Cancelled)));
    assert!(
        handle
            .live_combat()
            .await
            .expect("restored live state")
            .is_none()
    );
    std::fs::write(&input, "0|100\n2|300\n").expect("recovery source");
    assert_eq!(
        handle
            .import_log(&input)
            .await
            .expect("reuse after cancellation")
            .damage,
        400
    );
    complete_buffered_import(&handle, &input).await;
    std::fs::write(&input, "0|1\n".repeat(100_000)).expect("shutdown source");
    let job = handle
        .start_import(&input)
        .await
        .expect("import before shutdown");
    tokio::time::timeout(Duration::from_secs(5), host.shutdown())
        .await
        .expect("shutdown deadline")
        .expect("shutdown");
    assert!(matches!(job.wait().await, Err(Error::Cancelled)));
    assert!(matches!(handle.diagnostics().await, Err(Error::Closed)));
    let restarted = Host::start(deployment.config())
        .await
        .expect("restart after joined cleanup");
    restarted.shutdown().await.expect("finish restarted host");
    canceled_startup_releases_its_session(&deployment)
        .await
        .expect("startup cancellation");
}

async fn complete_buffered_import(host: &HostHandle, input: &Utf8Path) {
    const RECORDS: usize = 16_384;
    std::fs::write(input, "0|1\n".repeat(RECORDS)).expect("buffered source");
    let started = std::time::Instant::now();
    let result = host
        .import_log(input)
        .await
        .expect("complete all buffered batches");
    assert_eq!(result.lines_read, RECORDS as u64);
    assert_eq!(result.damage, RECORDS as i64);
    println!(
        "buffered import: {RECORDS} records in {:?}",
        started.elapsed()
    );
}

struct ReentrantAudio {
    host: HostHandle,
    disposed: Option<tokio::sync::oneshot::Sender<Result<(), String>>>,
}

impl aftercast_tts::Backend for ReentrantAudio {
    fn speak(
        &mut self,
        _: &aftercast_tts::Utterance,
        cancellation: &aftercast_tts::Cancellation,
    ) -> Result<(), aftercast_tts::Error> {
        cancellation.check()
    }
}

impl aftercast_tts::SoundBackend for ReentrantAudio {
    fn play(
        &mut self,
        _: &aftercast_tts::SoundRequest,
        cancellation: &aftercast_tts::Cancellation,
    ) -> Result<(), aftercast_tts::Error> {
        cancellation.check()
    }
}

impl Drop for ReentrantAudio {
    fn drop(&mut self) {
        // A backend may consult host state while releasing resources; the owner must keep dispatching.
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .map_err(|error| error.to_string())
            .and_then(|runtime| {
                runtime
                    .block_on(self.host.diagnostics())
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            });
        if let Some(disposed) = self.disposed.take() {
            drop(disposed.send(result));
        }
    }
}

async fn audio_cleanup_can_query_the_host(host: &Host) {
    let handle = host.handle();
    let (disposed, completion) = tokio::sync::oneshot::channel();
    let speaker = aftercast_tts::Speaker::new_async(move || {
        Ok(ReentrantAudio {
            host: handle,
            disposed: Some(disposed),
        })
    })
    .await
    .expect("reentrant backend");
    host.set_tts(speaker).await.expect("install backend");
    let handle = host.handle();
    let replacement = aftercast_tts::Speaker::new_async(move || {
        Ok(ReentrantAudio {
            host: handle,
            disposed: None,
        })
    })
    .await
    .expect("replacement backend");
    tokio::time::timeout(Duration::from_secs(2), host.set_tts(replacement))
        .await
        .expect("replacement blocked owner dispatch")
        .expect("replace backend");
    tokio::time::timeout(Duration::from_secs(2), host.clear_tts())
        .await
        .expect("audio cleanup deadlocked with a host query")
        .expect("clear backend");
    completion
        .await
        .expect("backend disposal finished")
        .expect("host query completed before cleanup returned");
    let handle = host.handle();
    let sounds = aftercast_tts::SoundPlayer::new_async(move || {
        Ok(ReentrantAudio {
            host: handle,
            disposed: None,
        })
    })
    .await
    .expect("effect backend");
    host.set_sound_player(sounds)
        .await
        .expect("install effect backend");
    tokio::time::timeout(Duration::from_secs(2), host.clear_sound_player())
        .await
        .expect("effect cleanup blocked owner dispatch")
        .expect("clear effects");
}

async fn canceled_startup_releases_its_session(deployment: &Deployment) -> Result<(), Error> {
    use std::future::Future;
    use std::task::Poll;
    let mut starting = Box::pin(Host::start(deployment.config()));
    let completed = std::future::poll_fn(|cx| {
        Poll::Ready(match starting.as_mut().poll(cx) {
            Poll::Pending => None,
            Poll::Ready(result) => Some(result),
        })
    })
    .await;
    drop(starting);
    if let Some(result) = completed {
        result
            .expect("startup raced cancellation")
            .shutdown()
            .await
            .expect("cleanup ready host");
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match Host::start(deployment.config()).await {
            Ok(host) => {
                host.shutdown()
                    .await
                    .expect("cleanup after canceled startup");
                break;
            }
            Err(Error::AlreadyActive) => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "canceled startup leaked the session"
                );
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
