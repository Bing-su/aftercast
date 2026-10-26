use aftercast::{Host, HostConfig};
use aftercast_tts::{Backend, Cancellation, SoundBackend, SoundRequest, Utterance};
use camino::Utf8Path;
use std::io::Write;
use std::time::Duration;

struct Recorder {
    file: std::fs::File,
    state: camino::Utf8PathBuf,
    speech: bool,
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if self.speech && self.state.join("audio.probe").exists() {
            std::fs::write(self.state.join("audio.retiring"), "waiting")
                .expect("retirement marker");
            while !self.state.join("audio.released").exists() {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

impl Recorder {
    fn record(&mut self, text: &str, volume: u8) -> Result<(), aftercast_tts::Error> {
        writeln!(self.file, "{volume}|{text}")
            .map_err(|error| aftercast_tts::Error::Backend(error.to_string()))?;
        if text == "fail" {
            return Err(aftercast_tts::Error::Backend(
                "fixture playback failure".into(),
            ));
        }
        Ok(())
    }
}

impl Backend for Recorder {
    fn speak(&mut self, request: &Utterance, _: &Cancellation) -> Result<(), aftercast_tts::Error> {
        self.record(&request.text, request.volume)
    }
}

impl SoundBackend for Recorder {
    fn play(
        &mut self,
        request: &SoundRequest,
        _: &Cancellation,
    ) -> Result<(), aftercast_tts::Error> {
        self.record(&request.source, request.volume)
    }
}

fn recorder(state: &Utf8Path, name: &str) -> Result<Recorder, aftercast_tts::Error> {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state.join(name))
        .map(|file| Recorder {
            file,
            state: state.to_owned(),
            speech: name == "tts.txt",
        })
        .map_err(|error| aftercast_tts::Error::Backend(error.to_string()))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Dispatch before the normal app branch, exactly as the future UI executable must do.
    if aftercast::run_child_if_requested_with_audio(
        |state| {
            let backend = recorder(state, "tts.txt")?;
            aftercast_tts::Speaker::new(|| Ok(backend))
        },
        |state| {
            let backend = recorder(state, "sound.txt")?;
            aftercast_tts::SoundPlayer::new(|| Ok(backend))
        },
    )? {
        return Ok(());
    }
    let mut args = std::env::args().skip(1);
    let root = args.next().ok_or("runtime root")?;
    let bridge = args.next().ok_or("bridge directory")?;
    let state = args.next().ok_or("state directory")?;
    // No executable override: this normal application branch must relaunch this exact file.
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let host = Host::start(
                HostConfig::builder()
                    .dotnet_root(root)
                    .bridge_directory(bridge)
                    .data_directory(&state)
                    .build(),
            )
            .await?;
            std::fs::write(
                std::path::Path::new(&state).join("app.started"),
                format!("{} {}", std::process::id(), host.process_id()),
            )?;
            host.shutdown().await?;
            Ok::<_, Box<dyn std::error::Error>>(())
        })
}
