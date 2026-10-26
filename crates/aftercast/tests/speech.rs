mod common;

use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use aftercast::{AudioSettings, Error, Host, PluginConfig};
use aftercast_tts::{
    Backend, Cancellation, SoundBackend, SoundPlayer, SoundRequest, Speaker, Utterance,
};
use camino::Utf8Path;
use common::{Deployment, deployment};

// The CLR bridge allows one active host per process, even when Rust runs tests on multiple threads.
static HOST_TEST: Mutex<()> = Mutex::new(());

struct Recorder(mpsc::Sender<String>);
impl Backend for Recorder {
    fn speak(&mut self, speech: &Utterance, _: &Cancellation) -> Result<(), aftercast_tts::Error> {
        self.0
            .send(speech.text.clone())
            .map_err(|e| aftercast_tts::Error::Backend(e.to_string()))?;
        if speech.text == "fail" {
            Err(aftercast_tts::Error::Backend(
                "fixture speech failure".into(),
            ))
        } else {
            Ok(())
        }
    }
}

struct AudioRecorder(mpsc::Sender<(String, u8)>);
impl Backend for AudioRecorder {
    fn speak(&mut self, speech: &Utterance, _: &Cancellation) -> Result<(), aftercast_tts::Error> {
        self.0
            .send((speech.text.clone(), speech.volume))
            .map_err(|e| aftercast_tts::Error::Backend(e.to_string()))?;
        Ok(())
    }
}

#[cfg(feature = "audio")]
struct NativeRecorder<B>(B, mpsc::Sender<()>);
#[cfg(feature = "audio")]
impl<B: Backend> Backend for NativeRecorder<B> {
    fn speak(
        &mut self,
        speech: &Utterance,
        cancellation: &Cancellation,
    ) -> Result<(), aftercast_tts::Error> {
        self.0.speak(speech, cancellation)?;
        self.1
            .send(())
            .map_err(|e| aftercast_tts::Error::Backend(e.to_string()))
    }
}
#[cfg(feature = "audio")]
impl<B: SoundBackend> SoundBackend for NativeRecorder<B> {
    fn play(
        &mut self,
        sound: &SoundRequest,
        cancellation: &Cancellation,
    ) -> Result<(), aftercast_tts::Error> {
        self.0.play(sound, cancellation)?;
        self.1
            .send(())
            .map_err(|e| aftercast_tts::Error::Backend(e.to_string()))
    }
}

#[cfg(all(feature = "audio", any(windows, target_os = "linux")))]
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires local speech service, audio device, .NET 10 and built fixture"]
async fn native_act_timer_speech_and_effects_finish_playback(deployment: Deployment) {
    let _host_test = HOST_TEST.lock().expect("serialize CLR hosts");
    let mut host = Host::start(deployment.config()).await.expect("host");
    let (sent, completed) = mpsc::channel();
    let speech_sent = sent.clone();
    let speaker = Speaker::new(move || {
        #[cfg(windows)]
        let backend = aftercast_tts::Sapi::new()?;
        #[cfg(target_os = "linux")]
        let backend = aftercast_tts::SpeechDispatcher::new()?;
        Ok(NativeRecorder(backend, speech_sent))
    })
    .expect("native speech backend");
    host.set_tts(speaker).await.expect("speech route");
    host.set_sound_player(
        SoundPlayer::new(|| Ok(NativeRecorder(aftercast_tts::FilePlayback, sent)))
            .expect("native file player"),
    )
    .await
    .expect("sound route");
    host.set_audio_settings(&AudioSettings {
        timer_expire: "tts expired".into(),
        ..AudioSettings::default()
    })
    .await
    .expect("settings");
    let plugin = host.create_plugin(PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll").canonicalize_utf8().expect("fixture path"))
        .type_name("Aftercast.PluginFixture.SoundPlugin").build()).await.expect("plugin");
    host.initialize_plugin(plugin).await.expect("init");
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut count = 0;
    while count < 4 {
        host.session_id()
            .await
            .expect("native playback or timer failure");
        count += completed.try_iter().count();
        assert!(
            Instant::now() < deadline,
            "native audio did not finish: {count}/4"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    host.shutdown().await.expect("stop native playback");
}
impl SoundBackend for AudioRecorder {
    fn play(&mut self, sound: &SoundRequest, _: &Cancellation) -> Result<(), aftercast_tts::Error> {
        self.0
            .send((sound.source.clone(), sound.volume))
            .map_err(|e| aftercast_tts::Error::Backend(e.to_string()))?;
        Ok(())
    }
}

struct FailingSound;
impl SoundBackend for FailingSound {
    fn play(&mut self, _: &SoundRequest, _: &Cancellation) -> Result<(), aftercast_tts::Error> {
        Err(aftercast_tts::Error::Backend(
            "fixture sound failure".into(),
        ))
    }
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and built fixture; run mise run test:plugins on Windows or Linux"]
async fn act_sound_and_timer_cues_cross_the_bridge_with_independent_volumes(
    deployment: Deployment,
) {
    let _host_test = HOST_TEST.lock().expect("serialize CLR hosts");
    let mut host = Host::start(deployment.config()).await.expect("host");
    let (tts_sent, tts) = mpsc::channel();
    let (sound_sent, sounds) = mpsc::channel();
    host.set_tts(Speaker::new(|| Ok(AudioRecorder(tts_sent))).expect("speaker"))
        .await
        .expect("TTS route");
    host.set_sound_player(SoundPlayer::new(|| Ok(AudioRecorder(sound_sent))).expect("player"))
        .await
        .expect("sound route");
    host.set_audio_settings(&AudioSettings {
        sound_volume: 61,
        tts_volume: 37,
        timer_expire: "tts expired".into(),
        ..AudioSettings::default()
    })
    .await
    .expect("settings");
    let plugin = host.create_plugin(PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll").canonicalize_utf8().expect("fixture path"))
        .type_name("Aftercast.PluginFixture.SoundPlugin").build()).await.expect("plugin");
    host.initialize_plugin(plugin)
        .await
        .expect("init sound caller");
    host.session_id().await.expect("warning");
    host.session_id().await.expect("expiry");
    let timeout = Duration::from_secs(2);
    assert_eq!(
        tts.recv_timeout(timeout).expect("start"),
        (" Caster".into(), 37)
    );
    assert_eq!(
        tts.recv_timeout(timeout).expect("expiry"),
        (" expired".into(), 37)
    );
    let (file, volume) = sounds.recv_timeout(timeout).expect("file sound");
    assert!(file.ends_with("fixture.wav"));
    assert_eq!(volume, 61);
    assert_eq!(
        sounds.recv_timeout(timeout).expect("warning beep"),
        ("beep".into(), 61)
    );
    host.set_sound_player(SoundPlayer::new(|| Ok(FailingSound)).expect("failing player"))
        .await
        .expect("replace sound route");
    for _ in 0..2 {
        let plugin = host.create_plugin(PluginConfig::builder()
            .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll").canonicalize_utf8().expect("fixture path"))
            .type_name("Aftercast.PluginFixture.SoundPlugin").build()).await.expect("repeat sound caller");
        host.initialize_plugin(plugin)
            .await
            .expect("sound acceptance");
    }
    let _deadline = Instant::now() + timeout;
    let mut failures = 0;
    while failures < 2 {
        match tokio::time::timeout(timeout, host.next_audio_error())
            .await
            .expect("effect error deadline")
        {
            Some(Error::Sound(aftercast_tts::Error::Backend(message))) => {
                assert_eq!(message, "fixture sound failure");
                failures += 1;
            }
            other => panic!("unexpected effect result: {other:?}"),
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    host.clear_sound_player().await.expect("clear sound");
    host.clear_tts().await.expect("clear speech");
    host.shutdown().await.expect("stop");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and built plugin fixture; run mise run test:plugins"]
async fn act_tts_reaches_injected_backend_and_reports_playback_failures(deployment: Deployment) {
    let _host_test = HOST_TEST.lock().expect("serialize CLR hosts");
    assert!(deployment.state.starts_with(deployment.directory()));
    let mut host = Host::start(deployment.config()).await.expect("start host");
    let (sender, received) = mpsc::channel();
    host.set_tts(Speaker::new(|| Ok(Recorder(sender))).expect("create backend"))
        .await
        .expect("configure TTS");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll")
        .canonicalize_utf8()
        .expect("fixture path");
    let good = host
        .create_plugin(
            PluginConfig::builder()
                .path(&path)
                .type_name("Aftercast.PluginFixture.SpeechPlugin")
                .build(),
        )
        .await
        .expect("create plugin");
    host.initialize_plugin(good)
        .await
        .expect("initialize plugin");
    host.session_id().await.expect("dispatch TTS");
    assert_eq!(
        received
            .recv_timeout(Duration::from_secs(2))
            .expect("real ACT call arrived"),
        "한글 <plain> & text"
    );
    let failed = host
        .create_plugin(
            PluginConfig::builder()
                .path(&path)
                .type_name("Aftercast.PluginFixture.FailingSpeechPlugin")
                .build(),
        )
        .await
        .expect("create failure fixture");
    host.initialize_plugin(failed)
        .await
        .expect("queue acceptance succeeds");
    let failed_again = host
        .create_plugin(
            PluginConfig::builder()
                .path(&path)
                .type_name("Aftercast.PluginFixture.FailingSpeechPlugin")
                .build(),
        )
        .await
        .expect("create second failure fixture");
    host.initialize_plugin(failed_again)
        .await
        .expect("second queue acceptance");
    let _until = Instant::now() + Duration::from_secs(2);
    let mut failures = 0;
    loop {
        match tokio::time::timeout(Duration::from_secs(2), host.next_audio_error())
            .await
            .expect("speech error deadline")
        {
            Some(Error::Speech(aftercast_tts::Error::Backend(message))) => {
                assert_eq!(message, "fixture speech failure");
                failures += 1;
                if failures == 2 {
                    break;
                }
            }
            other => panic!("expected eventual TTS failure, received {other:?}"),
        }
    }
    host.clear_tts().await.expect("disable TTS");
    host.shutdown().await.expect("stop host");
    let restarted = Host::start(deployment.config())
        .await
        .expect("restart host");
    let plugin = restarted
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.SpeechPlugin")
                .build(),
        )
        .await
        .expect("create after restart");
    assert!(
        restarted.initialize_plugin(plugin).await.is_err(),
        "restart must not retain an old speech route"
    );
    restarted.shutdown().await.expect("stop restarted host");
}
