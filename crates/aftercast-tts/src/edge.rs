use std::borrow::Cow;
use std::time::Duration;

use msedge_tts::tts::{
    SpeechConfig,
    stream::{SynthesizedResponse, tokio_runtime::msedge_tts_split_async},
};
use tokio::runtime::{Builder, Runtime};

use crate::{Backend, Cancellation, Error, SynthesizedAudio, Synthesizer, Utterance, failure};

/// Online Edge Read Aloud backend. Selection is explicit; no local backend falls back to this service.
pub struct Edge {
    voice: String,
    runtime: Runtime,
}

impl Edge {
    /// For example, `Edge::new("ko-KR-SunHiNeural")`. Construction does not connect to the network.
    pub fn new(voice: impl Into<String>) -> Result<Self, Error> {
        let voice = voice.into();
        validate_voice(&voice)?;
        Ok(Self {
            voice,
            runtime: Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(failure)?,
        })
    }

    /// Fetch unscaled MP3 bytes without an audio device; volume is applied by playback.
    pub fn synthesize_mp3(
        &mut self,
        utterance: &Utterance,
        cancellation: &Cancellation,
    ) -> Result<Vec<u8>, Error> {
        utterance.validate()?;
        cancellation.check()?;
        let voice = utterance.voice.as_deref().unwrap_or(&self.voice);
        validate_voice(voice)?;
        let config = SpeechConfig {
            voice_name: voice.into(),
            audio_format: "audio-24khz-48kbitrate-mono-mp3".into(),
            pitch: 0,
            rate: i32::from(utterance.rate),
            volume: 0,
        };
        // The dependency embeds strings in SSML; escape plain text here and restrict voice IDs to their documented form.
        let text = escape_text(&utterance.text);
        self.runtime.block_on(async {
            tokio::select! {
                result = tokio::time::timeout(Duration::from_secs(30), fetch(&text, &config)) => result.map_err(failure)?,
                result = async { loop { cancellation.check()?; tokio::time::sleep(Duration::from_millis(20)).await; } } => result,
            }
        })
    }
}

fn validate_voice(voice: &str) -> Result<(), Error> {
    if voice.is_empty()
        || voice.len() > 512
        || !voice
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(Error::InvalidRequest(
            "Edge voice must be a voice-list ID, e.g. ko-KR-SunHiNeural",
        ));
    }
    Ok(())
}

fn escape_text(text: &str) -> Cow<'_, str> {
    let extra: usize = text
        .bytes()
        .map(|byte| match byte {
            b'&' => 4,
            b'<' | b'>' => 3,
            b'"' | b'\'' => 5,
            _ => 0,
        })
        .sum();
    if extra == 0 {
        return Cow::Borrowed(text);
    }
    // Escape source characters once, e.g. an input &amp; must become &amp;amp; rather than escaping our own output.
    let mut escaped = String::with_capacity(text.len() + extra);
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    Cow::Owned(escaped)
}

async fn fetch(text: &str, config: &SpeechConfig) -> Result<Vec<u8>, Error> {
    let (mut sender, mut receiver) = msedge_tts_split_async().await.map_err(failure)?;
    sender.send(text, config).await.map_err(failure)?;
    let mut audio = Vec::new();
    while receiver.can_read().await {
        let response = receiver.read().await.map_err(failure)?;
        // The dependency returns None on EOF without clearing can_read; yield so timeout/cancel still runs on that path.
        tokio::task::yield_now().await;
        let Some(SynthesizedResponse::AudioBytes(bytes)) = response else {
            continue;
        };
        if audio.len().saturating_add(bytes.len()) > 16 * 1024 * 1024 {
            return Err(Error::Backend("Edge audio exceeds 16 MiB".into()));
        }
        audio.extend(bytes);
    }
    if audio.is_empty() {
        return Err(Error::Backend("Edge returned no audio".into()));
    }
    Ok(audio)
}

impl Backend for Edge {
    fn speak(&mut self, utterance: &Utterance, cancellation: &Cancellation) -> Result<(), Error> {
        let audio = Synthesizer::synthesize(self, utterance, cancellation)?;
        crate::playback::play(audio, utterance.volume, cancellation)
    }
}

impl Synthesizer for Edge {
    fn synthesize(
        &mut self,
        utterance: &Utterance,
        cancellation: &Cancellation,
    ) -> Result<SynthesizedAudio, Error> {
        self.synthesize_mp3(utterance, cancellation)
            .map(SynthesizedAudio::Mp3)
    }
}

#[cfg(test)]
mod tests {
    use super::{escape_text, validate_voice};
    use std::borrow::Cow;

    #[test]
    fn plain_text_cannot_inject_ssml_and_voice_ids_are_restricted() {
        assert_eq!(
            escape_text("한글 <voice> & '\""),
            "한글 &lt;voice&gt; &amp; &apos;&quot;"
        );
        assert_eq!(
            escape_text("&amp;<>&\"'🙂"),
            "&amp;amp;&lt;&gt;&amp;&quot;&apos;🙂"
        );
        for text in ["", "ordinary speech", "한글 🙂"] {
            assert!(matches!(escape_text(text), Cow::Borrowed(value) if value == text));
        }
        validate_voice("ko-KR-SunHiNeural").expect("valid service voice ID");
        assert!(matches!(
            validate_voice("ko-KR' /><audio src='remote"),
            Err(crate::Error::InvalidRequest(_))
        ));
    }
}
