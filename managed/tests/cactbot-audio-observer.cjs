// Observe real browser objects; keep native playback, rejection and speech callbacks intact.
function observeAudio() {
  const events = window.audioEvents = [];
  window.addEventListener('unhandledrejection', event => events.push({ type: 'rejection', error: String(event.reason) }));
  const play = HTMLMediaElement.prototype.play;
  HTMLMediaElement.prototype.play = function(...args) {
    const item = { type: 'media', src: this.src, volume: this.volume, playing: false, ended: false };
    events.push(item);
    this.addEventListener('playing', () => { item.playing = true; });
    this.addEventListener('ended', () => { item.ended = true; item.time = this.currentTime; item.duration = this.duration; });
    this.addEventListener('error', () => { item.error = this.error?.message || this.error?.code; });
    return play.apply(this, args);
  };
  if (window.speechSynthesis) {
    const synth = window.speechSynthesis;
    const speak = synth.speak.bind(synth);
    synth.speak = utterance => {
      const item = { type: 'speech', text: utterance.text, lang: utterance.lang, voice: utterance.voice?.name,
        local: utterance.voice?.localService, started: false, ended: false };
      events.push(item);
      utterance.addEventListener('start', () => { item.started = true; });
      utterance.addEventListener('end', event => { item.ended = true; item.elapsed = event.elapsedTime; });
      utterance.addEventListener('error', event => { item.error = event.error; });
      speak(utterance);
    };
  }
}
if (typeof module !== 'undefined') module.exports = observeAudio;
