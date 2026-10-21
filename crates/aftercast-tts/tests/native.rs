#[cfg(feature = "system")]
#[test]
#[ignore = "requires an installed local voice and working audio device"]
fn system_playback_completion_and_cancellation() {
    use aftercast_tts::{Error, Speaker, Utterance};
    use std::time::Duration;
    let speaker = Speaker::system().expect("initialize local system speech");
    speaker
        .speak(Utterance::new("Aftercast offline speech test."))
        .expect("enqueue")
        .wait()
        .expect("playback completes");
    let long = speaker
        .speak(Utterance::new("This speech will be cancelled. ".repeat(80)))
        .expect("enqueue long speech");
    std::thread::sleep(Duration::from_millis(100));
    long.cancel();
    assert!(matches!(long.wait(), Err(Error::Cancelled)));
    speaker
        .speak(Utterance::new("Speech resumed."))
        .expect("enqueue after cancellation")
        .wait()
        .expect("backend recovers");
}

#[cfg(feature = "edge")]
#[test]
#[ignore = "contacts the Edge online service; no audio device required"]
fn edge_synthesizes_real_mp3_and_rejects_precancelled_work() {
    use aftercast_tts::{Cancellation, Edge, Error, Utterance};

    let mut edge = Edge::new("ko-KR-SunHiNeural").expect("initialize");
    let cancel = Cancellation::default();
    cancel.cancel();
    assert!(matches!(
        edge.synthesize_mp3(&Utterance::new("cancelled"), &cancel),
        Err(Error::Cancelled)
    ));
    let audio = edge
        .synthesize_mp3(
            &Utterance::new("애프터캐스트 음성 테스트입니다. <태그> & 문자"),
            &Cancellation::default(),
        )
        .expect("online synthesis");
    assert!(audio.len() > 1000, "must receive real compressed audio");
    let during = Cancellation::default();
    let signal = during.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(10));
        signal.cancel();
    });
    let result = edge.synthesize_mp3(
        &Utterance::new("진행 중인 합성을 취소합니다. ".repeat(60)),
        &during,
    );
    canceller.join().expect("join cancellation helper");
    assert!(matches!(result, Err(Error::Cancelled)));
}

#[cfg(feature = "playback")]
#[test]
#[ignore = "requires a working audio device; plays silent fixture PCM, no model is loaded"]
fn pcm_synthesizer_uses_common_playback_and_cancellation() {
    use aftercast_tts::{
        Cancellation, Error, PcmAudio, Playback, Speaker, SynthesizedAudio, Synthesizer, Utterance,
    };

    struct PcmFixture;
    impl Synthesizer for PcmFixture {
        fn synthesize(
            &mut self,
            request: &Utterance,
            cancellation: &Cancellation,
        ) -> Result<SynthesizedAudio, Error> {
            cancellation.check()?;
            let samples = if request.text == "long" {
                240_000
            } else {
                6000
            };
            Ok(SynthesizedAudio::Pcm(PcmAudio::new(
                vec![0.0; samples],
                24_000,
                1,
            )?))
        }
    }
    let speaker = Speaker::new(|| Ok(Playback::new(PcmFixture))).expect("PCM fixture backend");
    speaker
        .speak(Utterance::new("short"))
        .expect("enqueue")
        .wait()
        .expect("PCM completion");
    let speech = speaker.speak(Utterance::new("long")).expect("enqueue");
    std::thread::sleep(std::time::Duration::from_millis(100));
    speech.cancel();
    assert!(matches!(speech.wait(), Err(Error::Cancelled)));
}
