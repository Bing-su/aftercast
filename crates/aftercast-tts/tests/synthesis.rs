use std::cell::Cell;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use aftercast_tts::{
    Backend, Cancellation, Error, PcmAudio, Speaker, SynthesizedAudio, Synthesizer, Utterance,
};

// A non-Send inference fixture proves the model can stay loaded on its creating worker; it is not a real TTS engine.
struct ModelFixture {
    calls: Rc<Cell<u32>>,
    owner: thread::ThreadId,
    observations: mpsc::Sender<(u32, String, thread::ThreadId)>,
}
impl Synthesizer for ModelFixture {
    fn synthesize(
        &mut self,
        request: &Utterance,
        cancellation: &Cancellation,
    ) -> Result<SynthesizedAudio, Error> {
        cancellation.check()?;
        if self.owner != thread::current().id() {
            return Err(Error::Backend("model moved between threads".into()));
        }
        self.calls.set(self.calls.get() + 1);
        self.observations
            .send((
                self.calls.get(),
                request.voice.clone().unwrap_or_default(),
                self.owner,
            ))
            .map_err(|e| Error::Backend(e.to_string()))?;
        Ok(SynthesizedAudio::Pcm(PcmAudio::new(
            vec![0.25, -0.25],
            24_000,
            1,
        )?))
    }
}
impl Drop for ModelFixture {
    fn drop(&mut self) {
        let _delivery =
            self.observations
                .send((self.calls.get(), "<drop>".into(), thread::current().id()));
    }
}

struct Inspect(Box<dyn Synthesizer>);
impl Backend for Inspect {
    fn speak(&mut self, request: &Utterance, cancellation: &Cancellation) -> Result<(), Error> {
        let audio = self.0.synthesize(request, cancellation)?;
        let SynthesizedAudio::Pcm(pcm) = audio else {
            return Err(Error::InvalidAudio("expected PCM"));
        };
        if (pcm.sample_rate(), pcm.channels()) != (24_000, 1) || pcm.samples() != [0.25, -0.25] {
            return Err(Error::InvalidAudio("model PCM layout changed"));
        }
        Ok(())
    }
}

#[test]
fn inference_trait_reuses_non_send_model_and_preserves_audio_without_a_device() {
    let (observations, received) = mpsc::channel();
    let caller = thread::current().id();
    let speaker = Speaker::new(move || {
        let model: Box<dyn Synthesizer> = Box::new(ModelFixture {
            calls: Rc::new(Cell::new(0)),
            owner: thread::current().id(),
            observations,
        });
        Ok(Box::new(Inspect(model)) as Box<dyn Backend>)
    })
    .expect("load model fixture once on worker");
    let mut worker = None;
    for count in [1, 2] {
        let mut request = Utterance::new("inference fixture");
        request.voice = Some("local-speaker-7".into());
        speaker
            .speak(request)
            .expect("enqueue")
            .wait()
            .expect("synthesized PCM");
        let (calls, voice, owner) = received.recv().expect("observation");
        assert_eq!((calls, voice.as_str()), (count, "local-speaker-7"));
        assert_ne!(owner, caller);
        worker = Some(owner);
    }
    drop(speaker);
    let (calls, event, owner) = received.recv().expect("model destruction observation");
    assert_eq!((calls, event.as_str()), (2, "<drop>"));
    assert_eq!(Some(owner), worker);
}

#[cfg(feature = "playback")]
#[test]
fn playback_accepts_non_send_models_without_opening_audio_during_load() {
    let (observations, _received) = mpsc::channel();
    let speaker = Speaker::new(move || {
        let model: Box<dyn Synthesizer> = Box::new(ModelFixture {
            calls: Rc::new(Cell::new(0)),
            owner: thread::current().id(),
            observations,
        });
        Ok(aftercast_tts::Playback::new(model))
    })
    .expect("construct playback around thread-affine inference state");
    drop(speaker);
}
