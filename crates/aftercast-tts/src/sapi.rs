use std::time::{Duration, Instant};

use windows::Win32::Media::Speech::{
    ISpeechObjectToken, ISpeechVoice, SVSFIsNotXML, SVSFPurgeBeforeSpeak, SVSFlagsAsync, SpVoice,
    SpeechVoiceSpeakFlags,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::core::BSTR;

use crate::{Backend, Cancellation, Error, Utterance, failure};

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: Each successful initialization owns one matching uninitialize on this same worker thread.
        unsafe {
            CoUninitialize();
        }
    }
}

/// Windows SAPI, using installed local voices. Construct through Speaker::new(Sapi::new).
pub struct Sapi {
    voice: ISpeechVoice,
    default_voice: ISpeechObjectToken,
    // Field order releases COM interfaces before their apartment.
    _apartment: Apartment,
}

impl Sapi {
    pub fn new() -> Result<Self, Error> {
        // SAFETY: No pointer is passed; the guard balances success on this thread.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(failure)?;
        let apartment = Apartment;
        // SAFETY: SpVoice is a registered COM class; its interface stays on the creating thread.
        let voice: ISpeechVoice =
            unsafe { CoCreateInstance(&SpVoice, None, CLSCTX_ALL) }.map_err(failure)?;
        // SAFETY: voice is a valid, thread-owned interface; the returned token has its own COM reference.
        let default_voice = unsafe { voice.Voice() }.map_err(failure)?;
        Ok(Self {
            voice,
            default_voice,
            _apartment: apartment,
        })
    }

    fn tokens(&self) -> Result<Vec<ISpeechObjectToken>, Error> {
        // SAFETY: Valid thread-owned voice and owned filter strings, with no retained borrowed pointer.
        let tokens =
            unsafe { self.voice.GetVoices(&BSTR::new(), &BSTR::new()) }.map_err(failure)?;
        // SAFETY: tokens owns the returned collection.
        let count = unsafe { tokens.Count() }.map_err(failure)?;
        (0..count)
            .map(|index| {
                // SAFETY: index is within the collection's reported count.
                unsafe { tokens.Item(index) }.map_err(failure)
            })
            .collect()
    }

    /// Installed SAPI token IDs, suitable for Utterance::voice.
    pub fn voices(&self) -> Result<Vec<String>, Error> {
        self.tokens()?
            .into_iter()
            .map(|token| {
                // SAFETY: token is a live interface on its owning thread; BSTR owns the returned string.
                unsafe { token.Id() }
                    .map(|id| id.to_string())
                    .map_err(failure)
            })
            .collect()
    }

    fn stop(&self) -> Result<(), Error> {
        // SAFETY: An empty owned BSTR with purge flags cancels this voice only; self owns the interface.
        unsafe {
            self.voice.Speak(
                &BSTR::new(),
                SpeechVoiceSpeakFlags(SVSFlagsAsync.0 | SVSFPurgeBeforeSpeak.0),
            )
        }
        .map(|_stream| ())
        .map_err(failure)
    }

    fn select_voice(&self, id: Option<&str>) -> Result<ISpeechObjectToken, Error> {
        let Some(id) = id else {
            return Ok(self.default_voice.clone());
        };
        for token in self.tokens()? {
            // SAFETY: Each enumerated token has its own live COM reference on this thread.
            let token_id = unsafe { token.Id() }.map_err(failure)?;
            if token_id.iter().copied().eq(id.encode_utf16()) {
                return Ok(token);
            }
        }
        Err(Error::Backend(
            "requested SAPI voice is not installed".into(),
        ))
    }
}

impl Backend for Sapi {
    fn speak(&mut self, utterance: &Utterance, cancellation: &Cancellation) -> Result<(), Error> {
        utterance.validate()?;
        cancellation.check()?;
        let selected = self.select_voice(utterance.voice.as_deref())?;
        // SAFETY: selected is a live token; COM takes its own reference if needed.
        unsafe { self.voice.putref_Voice(&selected) }.map_err(failure)?;
        // SAFETY: Utterance validation bounds rate to SAPI's -10..10 after conversion.
        unsafe { self.voice.SetRate(i32::from(utterance.rate) / 10) }.map_err(failure)?;
        // SAFETY: Utterance validation bounds volume to 0..100.
        unsafe { self.voice.SetVolume(i32::from(utterance.volume)) }.map_err(failure)?;
        // SAFETY: SAPI copies the bounded BSTR during Speak; plain-text flags prevent SSML interpretation.
        unsafe {
            self.voice.Speak(
                &BSTR::from(&utterance.text),
                SpeechVoiceSpeakFlags(SVSFlagsAsync.0 | SVSFIsNotXML.0 | SVSFPurgeBeforeSpeak.0),
            )
        }
        .map_err(failure)?;
        let started = Instant::now();
        loop {
            if cancellation.is_cancelled() || started.elapsed() > Duration::from_secs(300) {
                self.stop()?;
                return cancellation
                    .check()
                    .and(Err(Error::Backend("SAPI playback timed out".into())));
            }
            // SAFETY: This waits at most 20 ms on the thread-owned interface and returns an explicit completion flag.
            if unsafe { self.voice.WaitUntilDone(20) }
                .map_err(failure)?
                .as_bool()
            {
                return Ok(());
            }
        }
    }
}
