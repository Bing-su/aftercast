// Observe real browser objects; keep native playback, rejection and speech callbacks intact.
function observeAudio() {
  const events = window.audioEvents = [];
  window.addEventListener('unhandledrejection', event => events.push({ type: 'rejection', error: String(event.reason) }));
  const play = HTMLMediaElement.prototype.play;
  HTMLMediaElement.prototype.play = function(...args) {
    const item = { type: 'media', src: this.src, volume: this.volume, muted: this.muted, playing: false, ended: false };
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
      const item = { type: 'speech', text: utterance.text, volume: utterance.volume, lang: utterance.lang, voice: utterance.voice?.name,
        local: utterance.voice?.localService, started: false, ended: false };
      events.push(item);
      const start = () => { item.started = true; };
      const finish = event => {
        if (event.type === 'error') item.error = event.error;
        else { item.ended = true; item.elapsed = event.elapsedTime; }
        cleanup();
      };
      const cleanup = () => {
        // cactbot caches utterances; a later replay must not mutate this request's completed record.
        utterance.removeEventListener('start', start);
        utterance.removeEventListener('end', finish);
        utterance.removeEventListener('error', finish);
      };
      utterance.addEventListener('start', start, { once: true });
      utterance.addEventListener('end', finish);
      utterance.addEventListener('error', finish);
      try { speak(utterance); }
      catch (error) { cleanup(); item.error = String(error); throw error; }
    };
  }
}
if (typeof module !== 'undefined') module.exports = observeAudio;
