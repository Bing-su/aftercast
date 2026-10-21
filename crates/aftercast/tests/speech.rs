mod common;

use std::sync::mpsc;
use std::time::{Duration, Instant};

use aftercast::{Error, Host, PluginConfig};
use aftercast_tts::{Backend, Cancellation, Speaker, Utterance};
use camino::Utf8Path;
use common::{Deployment, deployment};

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

#[rstest::rstest]
#[ignore = "requires .NET 10 and built plugin fixture; run mise run test:plugins"]
fn act_tts_reaches_injected_backend_and_reports_playback_failures(deployment: Deployment) {
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
