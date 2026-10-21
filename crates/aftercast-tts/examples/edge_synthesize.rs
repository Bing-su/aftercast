#[cfg(feature = "edge")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use aftercast_tts::{Cancellation, Edge, Utterance};
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: edge_synthesize <output.mp3> <text> <voice>")?;
    let text = args.next().ok_or("text is required")?;
    let voice = args.next().ok_or("voice is required")?;
    let mut backend = Edge::new(voice)?;
    let bytes = backend.synthesize(&Utterance::new(text), &Cancellation::default())?;
    println!("Synthesized {} bytes", bytes.len());
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(not(feature = "edge"))]
fn main() {
    eprintln!("Enable the edge feature to synthesize online");
}
