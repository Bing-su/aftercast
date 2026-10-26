use aftercast::{Host, HostConfig};
use aftercast_tts::{Backend, Cancellation, SoundBackend, SoundRequest, Utterance};
use camino::Utf8Path;
use std::io::{Read, Write};
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
    if std::env::args().nth(1).as_deref() == Some("--aftercast-child")
        && let Ok(fault) = std::env::var("AFTERCAST_TEST_CHILD_FAULT")
    {
        return child_fault(&fault);
    }
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
    let startup_timeout = std::env::var("AFTERCAST_TEST_STARTUP_MS")
        .map_or(Ok(30_000), |value| value.parse::<u64>())?;
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
                    .startup_timeout(Duration::from_millis(startup_timeout))
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

fn child_fault(fault: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Faults consume the private launch frame, then deliberately violate the normal entry contract.
    let mut input = std::io::stdin().lock();
    let mut length = [0; 4];
    input.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length) as usize;
    if length > 65536 {
        return Err("test launch frame too large".into());
    }
    let mut bytes = vec![0; length];
    input.read_exact(&mut bytes)?;
    // Match the private launch field order without exposing test-only access to the host config.
    let ((token, config, _parent), remaining): (([u8; 32], HostConfig, u32), _) =
        postcard::take_from_bytes(&bytes)?;
    if !remaining.is_empty() {
        return Err("trailing launch bytes".into());
    }
    let state = std::env::var("AFTERCAST_TEST_STATE")?;
    std::fs::create_dir_all(&state)?;
    if fault == "omit-hook" {
        let marker = std::path::Path::new(&state).join("omitted.children");
        writeln!(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&marker)?,
            "{}",
            std::process::id()
        )?;
        // Bound the deliberately broken baseline: a grandchild is evidence, never spawn a third generation.
        if std::fs::read_to_string(marker)?.lines().count() > 1 {
            return Err("recursive child launch".into());
        }
        return tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                Host::start(config).await?.shutdown().await?;
                Ok(())
            });
    }
    if fault == "after-auth" {
        let mut stream = std::net::TcpStream::connect(std::env::var("AFTERCAST_WORKER_ENDPOINT")?)?;
        stream.write_all(&token)?;
        std::fs::write(
            std::path::Path::new(&state).join("authenticated.pid"),
            std::process::id().to_string(),
        )?;
        loop {
            std::thread::park();
        }
    }
    Err("unknown child fault".into())
}
