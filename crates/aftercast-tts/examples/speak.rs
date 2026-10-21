use aftercast_tts::{Speaker, Utterance};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let backend = args
        .next()
        .ok_or("usage: speak <system|edge> <text> [voice]")?;
    let text = args.next().ok_or("text is required")?;
    let voice = args.next();
    let speaker = match backend.as_str() {
        "system" => Speaker::system()?,
        #[cfg(feature = "edge")]
        "edge" => {
            let id = voice
                .clone()
                .ok_or("Edge requires a voice, e.g. ko-KR-SunHiNeural")?;
            Speaker::new(move || aftercast_tts::Edge::new(id))?
        }
        _ => return Err("unknown backend".into()),
    };
    let mut speech = Utterance::new(text);
    speech.voice = voice;
    speaker.speak(speech)?.wait()?;
    println!("Speech playback completed");
    Ok(())
}
