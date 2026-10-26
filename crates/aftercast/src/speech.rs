use std::collections::VecDeque;

use aftercast_tts::{SoundPlayer, SoundRequest, Speaker, Speech, Utterance};
use bytes::{BufMut, BytesMut};

use crate::host::CoreHost;
use crate::runtime::{ManagedSession, Request};
use crate::{Error, HostHandle};

pub(crate) struct AudioOutput<T> {
    pub(crate) backend: T,
    pending: VecDeque<Speech>,
}

impl<T> AudioOutput<T> {
    fn new(backend: T) -> Self {
        Self {
            backend,
            pending: VecDeque::new(),
        }
    }

    fn drain_completed(&mut self) -> Result<(), aftercast_tts::Error> {
        // FIFO completion preserves later failures for the next pump, e.g. two failed effects.
        while let Some(job) = self.pending.front() {
            let result = job.try_wait();
            if matches!(result, Ok(None)) {
                break;
            }
            self.pending.pop_front();
            result?;
        }
        Ok(())
    }

    fn pump(
        &mut self,
        session: &ManagedSession,
        command: u32,
        submit: impl Fn(&T, &str, u8) -> Result<Speech, aftercast_tts::Error>,
        map_error: fn(aftercast_tts::Error) -> Error,
    ) -> Result<(), Error> {
        self.drain_completed().map_err(map_error)?;
        if self.pending.len() >= 32 {
            return Ok(());
        }
        let mut request = packet(session.id(), command, 0, 4128);
        while self.pending.len() < 32 {
            session.request(Request::Speech, &mut request)?;
            let Some((text, volume)) = audio_request(&request)? else {
                break;
            };
            self.pending
                .push_back(submit(&self.backend, text, volume).map_err(map_error)?);
            // Reuse one packet per pump; clear outputs so the next dequeue has a valid request header.
            request
                .get_mut(20..32)
                .ok_or_else(|| Error::InvalidSpeechResponse("missing audio header".into()))?
                .fill(0);
        }
        Ok(())
    }
}

/// ACT audio volumes and default timer cues. Per-definition start/warning cues take precedence.
/// Cues accept `none`, `beep`, `tts`, `tts text`, or a local file path; e.g. `tts Ready`.
#[derive(Clone, Debug)]
pub struct AudioSettings {
    pub sound_volume: u8,
    pub tts_volume: u8,
    pub timer_start: String,
    pub timer_warning: String,
    pub timer_expire: String,
}
impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            sound_volume: 100,
            tts_volume: 100,
            timer_start: "none".into(),
            timer_warning: "none".into(),
            timer_expire: "none".into(),
        }
    }
}

fn packet(session: u64, command: u32, argument: u32, capacity: usize) -> BytesMut {
    let mut packet = BytesMut::with_capacity(capacity);
    packet.put_u32_le(capacity as u32);
    packet.put_u32_le(1);
    packet.put_u64_le(session);
    packet.put_u32_le(command);
    packet.put_u32_le(argument);
    packet.resize(capacity, 0);
    packet
}

impl CoreHost {
    /// Route PlaySound and timer effects to an independent worker, e.g. `SoundPlayer::system()?`.
    pub fn set_sound_player(&mut self, player: SoundPlayer) -> Result<(), Error> {
        self.session
            .request(Request::Speech, &mut packet(self.session.id(), 3, 1, 32))?;
        if let Some(previous) = self.sounds.replace(AudioOutput::new(player)) {
            previous.backend.shutdown_blocking().map_err(Error::Sound)?;
        }
        Ok(())
    }
    pub fn clear_sound_player(&mut self) -> Result<(), Error> {
        self.session
            .request(Request::Speech, &mut packet(self.session.id(), 3, 0, 32))?;
        if let Some(previous) = self.sounds.take() {
            previous.backend.shutdown_blocking().map_err(Error::Sound)?;
        }
        Ok(())
    }
    /// Route ACT TTS requests to an explicitly initialized backend before loading plugins.
    /// For example: `host.set_tts(aftercast_tts::Speaker::system()?)?`.
    /// Replacing the backend cancels its active and queued speech. No online fallback is selected.
    pub fn set_tts(&mut self, speaker: Speaker) -> Result<(), Error> {
        self.session
            .request(Request::Speech, &mut packet(self.session.id(), 1, 1, 32))?;
        if let Some(previous) = self.speech.replace(AudioOutput::new(speaker)) {
            previous.backend.shutdown_blocking()?;
        }
        Ok(())
    }

    /// Disable ACT speech and cancel active/queued playback, clearing both sides of the bridge.
    pub fn clear_tts(&mut self) -> Result<(), Error> {
        self.session
            .request(Request::Speech, &mut packet(self.session.id(), 1, 0, 32))?;
        if let Some(previous) = self.speech.take() {
            previous.backend.shutdown_blocking()?;
        }
        Ok(())
    }

    pub(crate) fn pump_speech(&mut self) -> Result<(), Error> {
        let Some(output) = &mut self.speech else {
            return Ok(());
        };
        output.pump(
            &self.session,
            2,
            |speaker, text, volume| {
                speaker.speak(Utterance {
                    volume,
                    ..Utterance::new(text)
                })
            },
            Error::from,
        )
    }

    pub(crate) fn pump_sounds(&mut self) -> Result<(), Error> {
        let Some(output) = &mut self.sounds else {
            return Ok(());
        };
        output.pump(
            &self.session,
            4,
            |player, source, volume| player.play(SoundRequest::new(source, volume)),
            Error::Sound,
        )
    }
}

fn audio_request(request: &[u8]) -> Result<Option<(&str, u8)>, Error> {
    let length = request
        .get(20..24)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| Error::InvalidSpeechResponse("missing audio length".into()))?
        as usize;
    if length == 0 {
        return Ok(None);
    }
    let text = request
        .get(32..)
        .and_then(|bytes| bytes.get(..length))
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .ok_or_else(|| Error::InvalidSpeechResponse("invalid audio text or length".into()))?;
    Ok(Some((text, volume(request)?)))
}

fn volume(request: &[u8]) -> Result<u8, Error> {
    request
        .get(24..28)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .and_then(|value| u8::try_from(value).ok())
        .filter(|value| *value <= 100)
        .ok_or_else(|| Error::InvalidSpeechResponse("invalid audio volume".into()))
}

impl HostHandle {
    pub async fn set_audio_settings(&self, settings: &AudioSettings) -> Result<(), Error> {
        let cues = [
            &settings.timer_start,
            &settings.timer_warning,
            &settings.timer_expire,
        ];
        if settings.sound_volume > 100
            || settings.tts_volume > 100
            || cues.iter().any(|cue| cue.chars().any(char::is_control))
        {
            return Err(aftercast_tts::Error::InvalidRequest("invalid audio settings").into());
        }
        let text = cues.map(|cue| cue.as_str()).join("\0");
        if text.len() > 4096 {
            return Err(
                aftercast_tts::Error::InvalidRequest("timer settings exceed 4096 bytes").into(),
            );
        }
        let mut request = packet(self.session, 5, text.len() as u32, 4128);
        let mut payload = request
            .get_mut(24..)
            .ok_or_else(|| Error::InvalidSpeechResponse("missing settings payload".into()))?;
        payload.put_u32_le(u32::from(settings.sound_volume));
        payload.put_u32_le(u32::from(settings.tts_volume));
        payload.put_slice(text.as_bytes());
        self.request(Request::Speech, &mut request).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    fn audio_responses_validate_text_capacity_and_volume() {
        let mut request = packet(1, 2, 3, 35);
        request
            .get_mut(24..28)
            .expect("volume")
            .copy_from_slice(&75_u32.to_le_bytes());
        request
            .get_mut(32..)
            .expect("text")
            .copy_from_slice("한".as_bytes());
        assert_eq!(
            audio_request(&request).expect("valid audio"),
            Some(("한", 75))
        );
        for length in 0..request.len() {
            audio_request(request.get(..length).expect("prefix"))
                .expect_err("reject truncated audio response");
        }
        request
            .get_mut(24..28)
            .expect("volume")
            .copy_from_slice(&101_u32.to_le_bytes());
        audio_request(&request).expect_err("reject volume above 100");
        request.get_mut(24..28).expect("volume").fill(0);
        request.get_mut(32..).expect("text").fill(255);
        audio_request(&request).expect_err("reject invalid UTF-8");
        request.get_mut(20..24).expect("length").fill(0);
        assert_eq!(audio_request(&request).expect("empty queue"), None);
    }
}
