use std::sync::mpsc;
use std::time::{Duration, Instant};

use ::speech_dispatcher::{Connection, DataMode, Mode, Notification, Priority, Voice};

use crate::{Backend, Cancellation, Error, Utterance, failure};

/// Linux Speech Dispatcher. The daemon and its configured local synthesis module must be installed.
pub struct SpeechDispatcher {
    connection: Connection,
    events: mpsc::Receiver<(u64, bool)>,
    default_voice: Voice,
}

impl SpeechDispatcher {
    pub fn new() -> Result<Self, Error> {
        let connection =
            Connection::open("aftercast", "tts", "", Mode::Threaded).map_err(failure)?;
        connection.set_data_mode(DataMode::Text).map_err(failure)?;
        let language = connection.get_language().map_err(failure)?;
        let voices = connection.list_synthesis_voices().map_err(failure)?;
        let default_voice = voices
            .iter()
            .find(|voice| voice.language.eq_ignore_ascii_case(language))
            .or_else(|| {
                voices
                    .iter()
                    .find(|voice| voice.language.split('-').next() == language.split('-').next())
            })
            .or_else(|| voices.first())
            .cloned()
            .ok_or_else(|| Error::Backend("Speech Dispatcher has no synthesis voices".into()))?;
        let (sender, events) = mpsc::channel();
        let cancelled = sender.clone();
        // Callbacks only send owned values; they never borrow the backend or call back into libspeechd.
        connection.on_end(Some(Box::new(move |id, _| {
            let _delivery = sender.send((id as u64, true));
        })));
        connection.on_cancel(Some(Box::new(move |id, _| {
            let _delivery = cancelled.send((id as u64, false));
        })));
        connection
            .set_notification_on(Notification::End)
            .map_err(failure)?;
        connection
            .set_notification_on(Notification::Cancel)
            .map_err(failure)?;
        Ok(Self {
            connection,
            events,
            default_voice,
        })
    }

    pub fn voices(&self) -> Result<Vec<String>, Error> {
        self.connection
            .list_synthesis_voices()
            .map(|voices| voices.into_iter().map(|voice| voice.name).collect())
            .map_err(failure)
    }
}

impl Backend for SpeechDispatcher {
    fn speak(&mut self, utterance: &Utterance, cancellation: &Cancellation) -> Result<(), Error> {
        utterance.validate()?;
        cancellation.check()?;
        // Each utterance has its own connection settings, so None must restore the backend's default voice.
        let voices = self.connection.list_synthesis_voices().map_err(failure)?;
        let voice = match &utterance.voice {
            Some(name) => voices.iter().find(|voice| voice.name == *name),
            None => Some(&self.default_voice),
        }
        .ok_or_else(|| Error::Backend("requested Speech Dispatcher voice is unavailable".into()))?;
        self.connection
            .set_synthesis_voice(voice)
            .map_err(failure)?;
        self.connection
            .set_voice_rate(i32::from(utterance.rate))
            .map_err(failure)?;
        self.connection
            .set_volume(i32::from(utterance.volume) * 2 - 100)
            .map_err(failure)?;
        let id = self
            .connection
            .say(Priority::Text, &utterance.text)
            .ok_or_else(|| Error::Backend("Speech Dispatcher rejected speech".into()))?;
        let started = Instant::now();
        loop {
            if cancellation.is_cancelled() || started.elapsed() > Duration::from_secs(300) {
                // Cancel this connection only: never stop another application's speech.
                self.connection.cancel().map_err(failure)?;
                return cancellation.check().and(Err(Error::Backend(
                    "Speech Dispatcher playback timed out".into(),
                )));
            }
            match self.events.recv_timeout(Duration::from_millis(20)) {
                Ok((event, true)) if event == id => return Ok(()),
                Ok((event, false)) if event == id => return Err(Error::Cancelled),
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(Error::Closed),
            }
        }
    }
}
