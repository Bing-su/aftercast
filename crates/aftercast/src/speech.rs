use std::collections::VecDeque;

use aftercast_tts::{SoundPlayer, SoundRequest, Speaker, Speech, Utterance};
use bytes::{BufMut, BytesMut};

use crate::runtime::Request;
use crate::{Error, Host};

pub(crate) struct SpeechOutput {
    speaker: Speaker,
    pending: VecDeque<Speech>,
}
pub(crate) struct SoundOutput {
    player: SoundPlayer,
    pending: VecDeque<Speech>,
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

impl Host {
    /// Route PlaySound and timer effects to an independent worker, e.g. `SoundPlayer::system()?`.
    pub fn set_sound_player(&mut self, player: SoundPlayer) -> Result<(), Error> {
        self.session
            .request(Request::Speech, &mut packet(self.session.id(), 3, 1, 32))?;
        self.sounds = Some(SoundOutput {
            player,
            pending: VecDeque::new(),
        });
        Ok(())
    }
    pub fn clear_sound_player(&mut self) -> Result<(), Error> {
        self.session
            .request(Request::Speech, &mut packet(self.session.id(), 3, 0, 32))?;
        self.sounds = None;
        Ok(())
    }
    /// Configure before loading plugins; settings are session-local and reset on stop.
    pub fn set_audio_settings(&mut self, settings: &AudioSettings) -> Result<(), Error> {
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
        let mut request = packet(self.session.id(), 5, text.len() as u32, 4128);
        let mut payload = request
            .get_mut(24..)
            .ok_or_else(|| Error::InvalidSpeechResponse("missing settings payload".into()))?;
        payload.put_u32_le(u32::from(settings.sound_volume));
        payload.put_u32_le(u32::from(settings.tts_volume));
        payload.put_slice(text.as_bytes());
        self.session.request(Request::Speech, &mut request)?;
        Ok(())
    }
    /// Route ACT TTS requests to an explicitly initialized backend before loading plugins.
    /// For example: `host.set_tts(aftercast_tts::Speaker::system()?)?`.
    /// Replacing the backend cancels its active and queued speech. No online fallback is selected.
    pub fn set_tts(&mut self, speaker: Speaker) -> Result<(), Error> {
        self.session
            .request(Request::Speech, &mut packet(self.session.id(), 1, 1, 32))?;
        self.speech = Some(SpeechOutput {
            speaker,
            pending: VecDeque::new(),
        });
        Ok(())
    }

    /// Disable ACT speech and cancel active/queued playback, clearing both sides of the bridge.
    pub fn clear_tts(&mut self) -> Result<(), Error> {
        self.session
            .request(Request::Speech, &mut packet(self.session.id(), 1, 0, 32))?;
        self.speech = None;
        Ok(())
    }

    pub(crate) fn pump_speech(&mut self) -> Result<(), Error> {
        let Some(output) = &mut self.speech else {
            return Ok(());
        };
        // Completions follow the worker's FIFO order; report each failure without consuming later failures.
        while let Some(speech) = output.pending.front() {
            let result = speech.try_wait();
            if matches!(result, Ok(None)) {
                break;
            }
            output.pending.pop_front();
            result?;
        }
        // At most 32 accepted jobs keeps completion state bounded even when a backend is slow.
        while output.pending.len() < 32 {
            let mut request = packet(self.session.id(), 2, 0, 4128);
            self.session.request(Request::Speech, &mut request)?;
            let length = request
                .get(20..24)
                .and_then(|bytes| bytes.try_into().ok())
                .map(u32::from_le_bytes)
                .ok_or_else(|| Error::InvalidSpeechResponse("missing text length".into()))?
                as usize;
            if length == 0 {
                break;
            }
            let bytes = request
                .get(32..)
                .and_then(|bytes| bytes.get(..length))
                .ok_or_else(|| {
                    Error::InvalidSpeechResponse("text exceeds response capacity".into())
                })?;
            let text = std::str::from_utf8(bytes)
                .map_err(|e| Error::InvalidSpeechResponse(e.to_string()))?;
            output.pending.push_back(output.speaker.speak(Utterance {
                volume: volume(&request)?,
                ..Utterance::new(text)
            })?);
        }
        Ok(())
    }

    pub(crate) fn pump_sounds(&mut self) -> Result<(), Error> {
        let Some(output) = &mut self.sounds else {
            return Ok(());
        };
        while let Some(job) = output.pending.front() {
            let result = job.try_wait();
            if matches!(result, Ok(None)) {
                break;
            }
            output.pending.pop_front();
            result?;
        }
        while output.pending.len() < 32 {
            let mut request = packet(self.session.id(), 4, 0, 4128);
            self.session.request(Request::Speech, &mut request)?;
            let length = request
                .get(20..24)
                .and_then(|bytes| bytes.try_into().ok())
                .map(u32::from_le_bytes)
                .ok_or_else(|| Error::InvalidSpeechResponse("missing sound length".into()))?
                as usize;
            if length == 0 {
                break;
            }
            let source = request
                .get(32..)
                .and_then(|bytes| bytes.get(..length))
                .and_then(|bytes| std::str::from_utf8(bytes).ok())
                .ok_or_else(|| Error::InvalidSpeechResponse("invalid sound source".into()))?;
            output.pending.push_back(
                output
                    .player
                    .play(SoundRequest::new(source, volume(&request)?))?,
            );
        }
        Ok(())
    }
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
