use aftercast_tts::{Cancellation, Error, SoundBackend, SoundPlayer, SoundRequest};
use std::sync::mpsc;

struct Recorder(mpsc::Sender<(String, u8)>);
impl SoundBackend for Recorder {
    fn play(&mut self, request: &SoundRequest, cancellation: &Cancellation) -> Result<(), Error> {
        cancellation.check()?;
        self.0
            .send((request.source.clone(), request.volume))
            .map_err(|e| Error::Backend(e.to_string()))?;
        if request.source == "bad" {
            Err(Error::InvalidAudio("fixture"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn sound_fifo_reports_failures_and_continues_at_the_requested_volume() {
    let (sent, received) = mpsc::channel();
    let player = SoundPlayer::new(|| Ok(Recorder(sent))).expect("sound worker");
    assert!(matches!(
        player.play(SoundRequest::new("", 100)),
        Err(Error::InvalidRequest(_))
    ));
    assert!(matches!(
        player.play(SoundRequest::new("beep", 101)),
        Err(Error::InvalidRequest(_))
    ));
    let bad = player.play(SoundRequest::new("bad", 17)).expect("accepted");
    let good = player
        .play(SoundRequest::new("beep", 61))
        .expect("accepted");
    assert!(matches!(bad.wait(), Err(Error::InvalidAudio(_))));
    good.wait().expect("recovery");
    assert_eq!(received.recv().expect("first"), ("bad".into(), 17));
    assert_eq!(received.recv().expect("second"), ("beep".into(), 61));
}

#[cfg(feature = "playback")]
#[test]
#[ignore = "requires a real default audio device; run on Windows and Linux"]
fn native_sound_file_uri_beep_completion_cancellation_and_decoder_failure() {
    let path = std::env::temp_dir().join(format!("aftercast-sound-{}.wav", std::process::id()));
    // Ten seconds of silence checks cancellation without producing a long audible test tone.
    let data_size = 24000u32 * 2 * 10;
    let mut wave = Vec::from(&b"RIFF"[..]);
    wave.extend((36 + data_size).to_le_bytes());
    wave.extend(b"WAVEfmt ");
    wave.extend(16u32.to_le_bytes());
    wave.extend(1u16.to_le_bytes());
    wave.extend(1u16.to_le_bytes());
    wave.extend(24000u32.to_le_bytes());
    wave.extend(48000u32.to_le_bytes());
    wave.extend(2u16.to_le_bytes());
    wave.extend(16u16.to_le_bytes());
    wave.extend(b"data");
    wave.extend(data_size.to_le_bytes());
    wave.resize(44 + data_size as usize, 0);
    std::fs::write(&path, &wave).expect("WAV fixture");
    let player = SoundPlayer::system().expect("native sound worker");
    let uri = url::Url::from_file_path(&path).expect("URI");
    let job = player
        .play(SoundRequest::new(uri.as_str(), 37))
        .expect("file accepted");
    std::thread::sleep(std::time::Duration::from_millis(200));
    job.cancel();
    assert!(matches!(job.wait(), Err(Error::Cancelled)));
    player
        .play(SoundRequest::new("beep", 10))
        .expect("beep accepted")
        .wait()
        .expect("beep completion");
    std::fs::write(&path, b"invalid audio").expect("corrupt fixture");
    assert!(
        player
            .play(SoundRequest::new(path.to_string_lossy(), 100))
            .expect("accepted corrupt file")
            .wait()
            .is_err()
    );
    std::fs::remove_file(path).expect("cleanup");
}
