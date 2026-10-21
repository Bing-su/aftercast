use crate::runtime::ManagedSession;
use crate::{Error, HostConfig};

/// A single active bridge lifetime on the calling thread.
///
/// This type is intentionally neither Send nor Sync. Stopping releases the ACT session,
/// while the CLR and loaded assemblies remain owned by the process.
///
/// ```compile_fail
/// fn requires_send<T: Send>() {}
/// requires_send::<aftercast::Host>();
/// ```
///
/// ```compile_fail
/// fn requires_sync<T: Sync>() {}
/// requires_sync::<aftercast::Host>();
/// ```
pub struct Host {
    pub(crate) session: ManagedSession,
    pub(crate) speech: Option<crate::speech::SpeechOutput>,
}

impl Host {
    /// Loads the installed runtime and starts the bundled managed session.
    ///
    /// Runtime and bridge paths must contain trusted files. The application-controlled data
    /// directory is created without overwriting existing contents. An incompatible CLR already
    /// loaded by another host is reported as a hosting error; no fallback runtime is started.
    pub fn start(config: HostConfig) -> Result<Self, Error> {
        ManagedSession::start(config).map(|session| Self {
            session,
            speech: None,
        })
    }

    /// Confirms that this session still owns the managed host and returns its identity.
    pub fn session_id(&self) -> Result<u64, Error> {
        self.session.call(2).map(|packet| packet.session)
    }

    /// Runs queued control callbacks and Windows UI messages on this host's creation thread.
    ///
    /// Call regularly from your application loop, for example before draining output. Each call
    /// starts at most 64 callbacks within a 5 ms scheduling budget; a callback itself can take
    /// longer. On Windows, also dispatches native messages; their callbacks are not counted in
    /// the return value or limited by the queue's scheduling budget. The caller must own an STA thread.
    pub fn pump(&mut self) -> Result<u32, Error> {
        let callbacks = self.session.call(4)?.runtime_major;
        self.pump_speech()?;
        Ok(callbacks)
    }

    /// Ends active combat, drains admitted actions and publishes its latest result.
    ///
    /// Live file callbacks are quiesced and resumed around completion. Call before importing while
    /// live combat is active, or to read a final result without waiting for another log timestamp.
    /// This is a no-op when idle; it does not flush a plugin's private deferred events or writer.
    pub fn end_combat(&mut self) -> Result<(), Error> {
        self.session.call(5).map(|_packet| ())
    }

    /// Stops the managed session. The CLR and loaded assemblies may remain in the process.
    pub fn stop(mut self) -> Result<(), Error> {
        self.speech.take();
        self.session.stop()
    }
}
