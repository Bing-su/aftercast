use std::io::Cursor;
use std::time::{Duration, Instant};

use msedge_tts::tts::{
    SpeechConfig,
    stream::{SynthesizedResponse, tokio_runtime::msedge_tts_split_async},
};
use tokio::runtime::{Builder, Runtime};

use crate::{Backend, Cancellation, Error, Utterance, failure};

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

    /// Fetch MP3 bytes without opening an audio device, useful to verify online synthesis in headless environments.
    pub fn synthesize(
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
            volume: i32::from(utterance.volume) - 100,
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

fn escape_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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
        let audio = self.synthesize(utterance, cancellation)?;
        cancellation.check()?;
        let stream = rodio::DeviceSinkBuilder::open_default_sink().map_err(failure)?;
        let player = rodio::play(stream.mixer(), Cursor::new(audio)).map_err(failure)?;
        let started = Instant::now();
        while !player.empty() {
            if cancellation.is_cancelled() {
                player.stop();
                return Err(Error::Cancelled);
            }
            if started.elapsed() > Duration::from_secs(300) {
                player.stop();
                return Err(Error::Backend("Edge playback timed out".into()));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{escape_text, validate_voice};

    #[test]
    fn plain_text_cannot_inject_ssml_and_voice_ids_are_restricted() {
        assert_eq!(
            escape_text("한글 <voice> & '\""),
            "한글 &lt;voice&gt; &amp; &apos;&quot;"
        );
        validate_voice("ko-KR-SunHiNeural").expect("valid service voice ID");
        assert!(matches!(
            validate_voice("ko-KR' /><audio src='remote"),
            Err(crate::Error::InvalidRequest(_))
        ));
    }
}
