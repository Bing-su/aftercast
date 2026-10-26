use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use aftercast_tts::{Backend, Cancellation, Error, Speaker, Utterance};

struct Recorder(mpsc::Sender<String>);
impl Backend for Recorder {
    fn speak(&mut self, speech: &Utterance, _: &Cancellation) -> Result<(), Error> {
        self.0
            .send(speech.text.clone())
            .map_err(|e| Error::Backend(e.to_string()))?;
        if speech.text == "fail" {
            Err(Error::Backend("fixture failure".into()))
        } else {
            Ok(())
        }
    }
}

struct Waiting(mpsc::Sender<()>);
impl Backend for Waiting {
    fn speak(&mut self, _: &Utterance, cancellation: &Cancellation) -> Result<(), Error> {
        self.0.send(()).map_err(|e| Error::Backend(e.to_string()))?;
        while !cancellation.is_cancelled() {
            thread::sleep(Duration::from_millis(1));
        }
        cancellation.check()
    }
}

#[test]
fn fifo_completion_and_backend_errors_are_observable() {
    let (sender, received) = mpsc::channel();
    let speaker = Speaker::new(|| Ok(Recorder(sender))).expect("initialize");
    let first = speaker
        .speak(Utterance::new("한글 <plain> & text"))
        .expect("enqueue");
    let second = speaker.speak(Utterance::new("fail")).expect("enqueue");
    first.wait().expect("first completion");
    assert!(matches!(second.wait(), Err(Error::Backend(message)) if message == "fixture failure"));
    assert_eq!(received.recv().expect("first"), "한글 <plain> & text");
    assert_eq!(received.recv().expect("second"), "fail");
}

#[test]
fn bounds_and_cancel_all_cover_active_and_waiting_requests() {
    let (sender, active) = mpsc::channel();
    let speaker = Speaker::new(|| Ok(Waiting(sender))).expect("initialize");
    let current = speaker.speak(Utterance::new("active")).expect("enqueue");
    active
        .recv_timeout(Duration::from_secs(2))
        .expect("worker started");
    let pending: Vec<_> = (0..32)
        .map(|_| {
            speaker
                .speak(Utterance::new("queued"))
                .expect("queue capacity")
        })
        .collect();
    assert!(matches!(
        speaker.speak(Utterance::new("overflow")),
        Err(Error::QueueFull)
    ));
    speaker.cancel_all();
    assert!(matches!(current.wait(), Err(Error::Cancelled)));
    for speech in pending {
        assert!(matches!(speech.wait(), Err(Error::Cancelled)));
    }
    assert!(
        active.try_recv().is_err(),
        "cancelled queued work must never reach the backend"
    );
    let next = speaker
        .speak(Utterance::new("after cancel"))
        .expect("new request");
    active
        .recv_timeout(Duration::from_secs(2))
        .expect("later request starts");
    next.cancel();
    assert!(matches!(next.wait(), Err(Error::Cancelled)));
}

#[test]
fn drop_cancels_active_and_queued_work_and_joins() {
    let (sender, active) = mpsc::channel();
    let speaker = Speaker::new(|| Ok(Waiting(sender))).expect("initialize");
    let current = speaker.speak(Utterance::new("active")).expect("enqueue");
    active
        .recv_timeout(Duration::from_secs(2))
        .expect("worker started");
    let pending = speaker.speak(Utterance::new("queued")).expect("enqueue");
    drop(speaker);
    assert!(matches!(current.wait(), Err(Error::Cancelled)));
    assert!(matches!(pending.wait(), Err(Error::Cancelled)));
}

#[test]
fn validation_precedes_backend_work_and_initialization_failure_is_returned() {
    let (sender, received) = mpsc::channel();
    let speaker = Speaker::new(|| Ok(Recorder(sender))).expect("initialize");
    for text in ["".into(), " \t\n".into(), "x\0y".into(), "x".repeat(4097)] {
        assert!(matches!(
            speaker.speak(Utterance::new(text)),
            Err(Error::InvalidRequest(_))
        ));
    }
    let mut invalid = Utterance::new("valid");
    invalid.rate = 101;
    assert!(matches!(
        speaker.speak(invalid),
        Err(Error::InvalidRequest(_))
    ));
    received
        .try_recv()
        .expect_err("invalid requests must not reach backend");
    assert!(matches!(
        Speaker::new(|| Err::<Recorder, _>(Error::Backend("initialization".into()))),
        Err(Error::Backend(_))
    ));
}

#[tokio::test]
async fn asynchronous_completion_and_shutdown_wait_for_backend_disposal() {
    let caller = thread::current().id();
    let (disposed, disposal) = tokio::sync::oneshot::channel();
    let speaker = Speaker::new_async(|| Ok(OwnedBackend(Some(disposed))))
        .await
        .expect("owned backend");
    speaker
        .speak(Utterance::new("async"))
        .expect("accepted")
        .wait_async()
        .await
        .expect("completed");
    speaker.shutdown().await.expect("native cleanup");
    assert_ne!(
        disposal
            .await
            .expect("backend was disposed before shutdown returned"),
        caller
    );
}

#[tokio::test]
async fn dropping_speaker_does_not_wait_for_a_blocked_backend() {
    let (release, blocked) = mpsc::channel();
    let (started, active) = tokio::sync::oneshot::channel();
    let speaker = Speaker::new(|| Ok(Blocked(blocked, Some(started)))).expect("blocked backend");
    let job = speaker.speak(Utterance::new("blocked")).expect("accepted");
    active.await.expect("backend entered");
    // Releasing only after Drop proves it cannot join the backend thread.
    drop(speaker);
    release.send(()).expect("release backend");
    assert!(matches!(job.wait_async().await, Err(Error::Cancelled)));
}

struct OwnedBackend(Option<tokio::sync::oneshot::Sender<thread::ThreadId>>);
impl Backend for OwnedBackend {
    fn speak(&mut self, _: &Utterance, _: &Cancellation) -> Result<(), Error> {
        Ok(())
    }
}
impl Drop for OwnedBackend {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _receiver_dropped = sender.send(thread::current().id());
        }
    }
}

struct Blocked(
    std::sync::mpsc::Receiver<()>,
    Option<tokio::sync::oneshot::Sender<()>>,
);
impl Backend for Blocked {
    fn speak(&mut self, _: &Utterance, cancellation: &Cancellation) -> Result<(), Error> {
        if let Some(sender) = self.1.take() {
            let _receiver_dropped = sender.send(());
        }
        self.0.recv().map_err(|_closed| Error::Closed)?;
        cancellation.check()
    }
}
