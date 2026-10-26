mod common;

use aftercast::{AudioSettings, Error, Host, PluginConfig};
use aftercast_tts::{Backend, Cancellation, SoundBackend, SoundRequest, Utterance};
use camino::Utf8Path;
use common::{Deployment, deployment};
use std::io::Write;
use std::time::Duration;

struct Recorder {
    file: std::fs::File,
    state: camino::Utf8PathBuf,
    speech: bool,
}
impl Drop for Recorder {
    fn drop(&mut self) {
        if self.speech && self.state.join("audio.probe").exists() {
            std::fs::write(self.state.join("audio.retiring"), "waiting")
                .expect("retirement marker");
            while !self.state.join("audio.released").exists() {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}
impl Recorder {
    fn record(&mut self, text: &str, volume: u8) -> Result<(), aftercast_tts::Error> {
        writeln!(self.file, "{volume}|{text}")
            .map_err(|error| aftercast_tts::Error::Backend(error.to_string()))?;
        if text == "fail" {
            return Err(aftercast_tts::Error::Backend(
                "fixture playback failure".into(),
            ));
        }
        Ok(())
    }
}
impl Backend for Recorder {
    fn speak(&mut self, request: &Utterance, _: &Cancellation) -> Result<(), aftercast_tts::Error> {
        self.record(&request.text, request.volume)
    }
}
impl SoundBackend for Recorder {
    fn play(
        &mut self,
        request: &SoundRequest,
        _: &Cancellation,
    ) -> Result<(), aftercast_tts::Error> {
        self.record(&request.source, request.volume)
    }
}
fn recorder(state: &Utf8Path, name: &str) -> Result<Recorder, aftercast_tts::Error> {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state.join(name))
        .map(|file| Recorder {
            file,
            state: state.to_owned(),
            speech: name == "tts.txt",
        })
        .map_err(|error| aftercast_tts::Error::Backend(error.to_string()))
}

#[test]
fn audio_worker_entry() {
    // Running this test binary as the worker keeps injected backends inside the isolation boundary.
    if std::env::var_os("AFTERCAST_WORKER_ENDPOINT").is_some() {
        aftercast::run_worker_with_audio(
            |state| {
                let backend = recorder(state, "tts.txt")?;
                aftercast_tts::Speaker::new(|| Ok(backend))
            },
            |state| {
                let backend = recorder(state, "sound.txt")?;
                aftercast_tts::SoundPlayer::new(|| Ok(backend))
            },
        )
        .expect("audio worker");
    }
}

async fn plugin(host: &Host, name: &str) {
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name(format!("Aftercast.PluginFixture.{name}"))
                .build(),
        )
        .await
        .expect("plugin");
    host.initialize_plugin(id).await.expect("initialize");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn audio_is_created_played_and_disposed_in_the_worker(deployment: Deployment) {
    assert!(deployment.state.starts_with(deployment.directory()));
    let executable = std::env::current_exe().expect("test worker path");
    let worker = Utf8Path::from_path(&executable).expect("UTF-8 test path");
    let mut host = Host::start(deployment.config_with_worker(worker))
        .await
        .expect("host");
    assert_ne!(host.process_id(), std::process::id());
    host.set_tts(true).await.expect("TTS");
    host.set_sound_player(true).await.expect("sound");
    host.set_audio_settings(&AudioSettings {
        tts_volume: 37,
        sound_volume: 61,
        timer_expire: "tts expired".into(),
        ..AudioSettings::default()
    })
    .await
    .expect("settings");
    plugin(&host, "SpeechPlugin").await;
    plugin(&host, "SoundPlugin").await;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let tts = std::fs::read_to_string(deployment.state.join("tts.txt")).expect("TTS records");
        let sound =
            std::fs::read_to_string(deployment.state.join("sound.txt")).expect("sound records");
        if tts.contains("37| expired") && sound.contains("61|beep") {
            assert!(tts.contains("37|한글 <plain> & text"));
            assert!(tts.contains("37| Caster"));
            assert!(sound.contains("fixture.wav"));
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "audio/timer delivery stalled"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    for _ in 0..2 {
        plugin(&host, "FailingSpeechPlugin").await;
    }
    for _ in 0..2 {
        let error = tokio::time::timeout(Duration::from_secs(5), host.next_audio_error())
            .await
            .expect("audio failure deadline")
            .expect("audio error");
        assert!(matches!(error, Error::Speech(_)), "{error:?}");
        assert!(error.to_string().contains("fixture playback failure"));
    }
    plugin(&host, "AudioCleanupPlugin").await;
    std::fs::write(deployment.state.join("audio.probe"), "enabled")
        .expect("enable retirement probe");
    tokio::time::timeout(Duration::from_secs(2), host.clear_tts())
        .await
        .expect("audio retirement must keep pumping owner callbacks")
        .expect("disable TTS");
    host.clear_sound_player().await.expect("disable sound");
    host.shutdown().await.expect("cleanup and reap");
    let restarted = Host::start(deployment.config()).await.expect("restart");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = restarted
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.SpeechPlugin")
                .build(),
        )
        .await
        .expect("plugin");
    assert!(
        restarted.initialize_plugin(id).await.is_err(),
        "audio routes must not survive restart"
    );
    restarted.shutdown().await.expect("restart cleanup");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn shutdown_retires_audio_before_disposing_its_owner_context(deployment: Deployment) {
    let executable = std::env::current_exe().expect("test worker path");
    let worker = Utf8Path::from_path(&executable).expect("UTF-8 test path");
    let host = Host::start(deployment.config_with_worker(worker))
        .await
        .expect("host");
    host.set_tts(true).await.expect("TTS");
    plugin(&host, "AudioCleanupPlugin").await;
    std::fs::write(deployment.state.join("audio.probe"), "enabled").expect("probe");
    tokio::time::timeout(Duration::from_secs(3), host.shutdown())
        .await
        .expect("shutdown must keep pumping audio cleanup")
        .expect("graceful cleanup");
    assert!(
        deployment.state.join("audio.released").exists(),
        "cleanup required an owner callback"
    );
}
