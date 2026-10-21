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
#[ignore = "requires local speech service, audio device, .NET 10 and built fixture"]
fn native_act_timer_speech_and_effects_finish_playback(deployment: Deployment) {
    let _host_test = HOST_TEST.lock().expect("serialize CLR hosts");
    let mut host = Host::start(deployment.config()).expect("host");
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
    host.set_tts(speaker).expect("speech route");
    host.set_sound_player(
        SoundPlayer::new(|| Ok(NativeRecorder(aftercast_tts::FilePlayback, sent)))
            .expect("native file player"),
    )
    .expect("sound route");
    host.set_audio_settings(&AudioSettings {
        timer_expire: "tts expired".into(),
        ..AudioSettings::default()
    })
    .expect("settings");
    let plugin = host.create_plugin(PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll").canonicalize_utf8().expect("fixture path"))
        .type_name("Aftercast.PluginFixture.SoundPlugin").build()).expect("plugin");
    host.initialize_plugin(plugin).expect("init");
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut count = 0;
    while count < 4 {
        host.pump().expect("native playback or timer failure");
        count += completed.try_iter().count();
        assert!(
            Instant::now() < deadline,
            "native audio did not finish: {count}/4"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    host.stop().expect("stop native playback");
}
impl SoundBackend for AudioRecorder {
    fn play(&mut self, sound: &SoundRequest, _: &Cancellation) -> Result<(), aftercast_tts::Error> {
        self.0
            .send((sound.source.clone(), sound.volume))
            .map_err(|e| aftercast_tts::Error::Backend(e.to_string()))?;
        Ok(())
    }
}

#[rstest::rstest]
#[ignore = "requires .NET 10 and built fixture; run mise run test:plugins on Windows or Linux"]
fn act_sound_and_timer_cues_cross_the_bridge_with_independent_volumes(deployment: Deployment) {
    let _host_test = HOST_TEST.lock().expect("serialize CLR hosts");
    let mut host = Host::start(deployment.config()).expect("host");
    let (tts_sent, tts) = mpsc::channel();
    let (sound_sent, sounds) = mpsc::channel();
    host.set_tts(Speaker::new(|| Ok(AudioRecorder(tts_sent))).expect("speaker"))
        .expect("TTS route");
    host.set_sound_player(SoundPlayer::new(|| Ok(AudioRecorder(sound_sent))).expect("player"))
        .expect("sound route");
    host.set_audio_settings(&AudioSettings {
        sound_volume: 61,
        tts_volume: 37,
        timer_expire: "tts expired".into(),
        ..AudioSettings::default()
    })
    .expect("settings");
    let plugin = host.create_plugin(PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll").canonicalize_utf8().expect("fixture path"))
        .type_name("Aftercast.PluginFixture.SoundPlugin").build()).expect("plugin");
    host.initialize_plugin(plugin).expect("init sound caller");
    host.pump().expect("warning");
    host.pump().expect("expiry");
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
    host.clear_sound_player().expect("clear sound");
    host.clear_tts().expect("clear speech");
    host.stop().expect("stop");
}

#[rstest::rstest]
#[ignore = "requires .NET 10 and built plugin fixture; run mise run test:plugins"]
fn act_tts_reaches_injected_backend_and_reports_playback_failures(deployment: Deployment) {
    let _host_test = HOST_TEST.lock().expect("serialize CLR hosts");
    assert!(deployment.state.starts_with(deployment.directory()));
    let mut host = Host::start(deployment.config()).expect("start host");
    let (sender, received) = mpsc::channel();
    host.set_tts(Speaker::new(|| Ok(Recorder(sender))).expect("create backend"))
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
        .expect("create plugin");
    host.initialize_plugin(good).expect("initialize plugin");
    host.pump().expect("dispatch TTS");
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
        .expect("create failure fixture");
    host.initialize_plugin(failed)
        .expect("queue acceptance succeeds");
    let failed_again = host
        .create_plugin(
            PluginConfig::builder()
                .path(&path)
                .type_name("Aftercast.PluginFixture.FailingSpeechPlugin")
                .build(),
        )
        .expect("create second failure fixture");
    host.initialize_plugin(failed_again)
        .expect("second queue acceptance");
    let until = Instant::now() + Duration::from_secs(2);
    let mut failures = 0;
    loop {
        match host.pump() {
            Err(Error::Speech(aftercast_tts::Error::Backend(message))) => {
                assert_eq!(message, "fixture speech failure");
                failures += 1;
                if failures == 2 {
                    break;
                }
            }
            Ok(_) if Instant::now() < until => std::thread::sleep(Duration::from_millis(1)),
            other => panic!("expected eventual TTS failure, received {other:?}"),
        }
    }
    host.clear_tts().expect("disable TTS");
    host.stop().expect("stop host");
    let mut restarted = Host::start(deployment.config()).expect("restart host");
    let plugin = restarted
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.SpeechPlugin")
                .build(),
        )
        .expect("create after restart");
    assert!(
        restarted.initialize_plugin(plugin).is_err(),
        "restart must not retain an old speech route"
    );
    restarted.stop().expect("stop restarted host");
}
