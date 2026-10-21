use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak, mpsc};
use std::thread::{self, JoinHandle};

use crate::Error;

/// Cooperative cancellation shared with a backend. For example, check between audio chunks.
#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn check(&self) -> Result<(), Error> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// One accepted request, including its eventual playback result. Dropping the
/// handle does not cancel speech; use cancel, Speaker::cancel_all, or drop Speaker.
pub struct Speech {
    result: mpsc::Receiver<Result<(), Error>>,
    cancellation: Cancellation,
}

impl Speech {
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }
    pub fn wait(self) -> Result<(), Error> {
        self.result.recv().map_err(|_disconnected| Error::Closed)?
    }
    /// Consume a ready result once; subsequent calls report Closed.
    pub fn try_wait(&self) -> Result<Option<()>, Error> {
        match self.result.try_recv() {
            Ok(result) => result.map(Some),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(Error::Closed),
        }
    }
}

struct Work<T> {
    request: T,
    result: mpsc::SyncSender<Result<(), Error>>,
    cancellation: Cancellation,
}

pub(crate) struct Worker<T> {
    sender: Option<mpsc::SyncSender<Work<T>>>,
    pending: Mutex<Vec<Weak<AtomicBool>>>,
    worker: Option<JoinHandle<()>>,
}

impl<T: Send + 'static> Worker<T> {
    pub(crate) fn new<B: 'static>(
        factory: impl FnOnce() -> Result<B, Error> + Send + 'static,
        perform: fn(&mut B, &T, &Cancellation) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        let (sender, receiver) = mpsc::sync_channel::<Work<T>>(32);
        let (ready, initialized) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("aftercast-tts".into())
            .spawn(move || {
                let mut backend = match factory() {
                    Ok(backend) => {
                        drop(ready.send(Ok(())));
                        backend
                    }
                    Err(error) => {
                        drop(ready.send(Err(error)));
                        return;
                    }
                };
                for work in receiver {
                    let result = work
                        .cancellation
                        .check()
                        .and_then(|()| perform(&mut backend, &work.request, &work.cancellation));
                    work.cancellation.0.store(true, Ordering::Release);
                    drop(work.result.send(result));
                }
            })
            .map_err(|e| Error::Backend(e.to_string()))?;
        if let Err(error) = initialized
            .recv()
            .map_err(|_disconnected| Error::Closed)
            .and_then(|r| r)
        {
            drop(worker.join());
            return Err(error);
        }
        Ok(Self {
            sender: Some(sender),
            pending: Mutex::new(Vec::new()),
            worker: Some(worker),
        })
    }

    pub(crate) fn submit(&self, request: T) -> Result<Speech, Error> {
        let cancellation = Cancellation(Arc::new(AtomicBool::new(false)));
        let (result, receiver) = mpsc::sync_channel(1);
        let mut pending = self.pending.lock().map_err(|_poisoned| Error::Closed)?;
        // Serialize admission with cancel_all so an accepted request cannot escape a concurrent cancellation.
        self.sender
            .as_ref()
            .ok_or(Error::Closed)?
            .try_send(Work {
                request,
                result,
                cancellation: cancellation.clone(),
            })
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => Error::QueueFull,
                mpsc::TrySendError::Disconnected(_) => Error::Closed,
            })?;
        pending.retain(|flag| {
            flag.upgrade()
                .is_some_and(|flag| !flag.load(Ordering::Acquire))
        });
        pending.push(Arc::downgrade(&cancellation.0));
        Ok(Speech {
            result: receiver,
            cancellation,
        })
    }
}

impl<T> Worker<T> {
    pub(crate) fn cancel_all(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            for flag in pending.drain(..).filter_map(|flag| flag.upgrade()) {
                flag.store(true, Ordering::Release);
            }
        }
    }
}

impl<T> Drop for Worker<T> {
    fn drop(&mut self) {
        self.cancel_all();
        self.sender.take();
        // Built-in backends poll cancellation; join also guarantees their native resources stay on the owning thread.
        if let Some(worker) = self.worker.take() {
            drop(worker.join());
        }
    }
}
