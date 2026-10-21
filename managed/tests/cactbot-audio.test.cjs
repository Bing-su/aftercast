const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { test } = require('node:test');
const assert = require('node:assert/strict');

function fixture(speak = () => {}) {
  const window = new EventTarget();
  window.speechSynthesis = { speak };
  class Media extends EventTarget { play() { return this.result; } }
  const context = { window, HTMLMediaElement: Media, module: { exports: {} } };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, 'cactbot-audio-observer.cjs'), 'utf8'), context);
  context.module.exports();
  return { window, Media };
}

test('cached cactbot utterances retain separate completion and failure records', () => {
  let calls = 0;
  const { window } = fixture(() => { calls++; });
  const utterance = new EventTarget();
  utterance.text = '4';
  window.speechSynthesis.speak(utterance);
  utterance.dispatchEvent(new Event('start'));
  utterance.dispatchEvent(Object.assign(new Event('end'), { elapsedTime: 1 }));
  window.speechSynthesis.speak(utterance);
  utterance.dispatchEvent(new Event('start'));
  utterance.dispatchEvent(Object.assign(new Event('error'), { error: 'canceled' }));
  assert.equal(calls, 2);
  assert.equal(window.audioEvents[0].elapsed, 1);
  assert.equal(window.audioEvents[0].error, undefined);
  assert.equal(window.audioEvents[1].error, 'canceled');
  assert.equal(window.audioEvents[1].ended, false);
});

test('native speech errors and media rejections are preserved instead of reporting success', async () => {
  const failure = new Error('native speech unavailable');
  const { window, Media } = fixture(() => { throw failure; });
  assert.throws(() => window.speechSynthesis.speak(new EventTarget()), error => error === failure);
  assert.match(window.audioEvents[0].error, /native speech unavailable/);
  const media = new Media();
  media.result = Promise.reject(new Error('autoplay denied'));
  assert.equal(media.play(), media.result);
  await assert.rejects(media.result, /autoplay denied/);
  assert.equal(window.audioEvents[1].ended, false);
});
