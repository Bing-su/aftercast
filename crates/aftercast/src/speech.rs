use std::collections::VecDeque;

use aftercast_tts::{Speaker, Speech, Utterance};
use bytes::{BufMut, BytesMut};

use crate::runtime::Request;
use crate::{Error, Host};

pub(crate) struct SpeechOutput {
    speaker: Speaker,
    pending: VecDeque<Speech>,
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
            output
                .pending
                .push_back(output.speaker.speak(Utterance::new(text))?);
        }
        Ok(())
    }
}
