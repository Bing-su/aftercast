const fs = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const { chromium } = require('playwright');
const { serveCactbot, installFixtureApi, triggerResponse } = require('./cactbot-fixture.cjs');
const observeAudio = require('./cactbot-audio-observer.cjs');

async function main() {
  const root = path.resolve(process.argv[2]);
  const output = path.resolve(process.argv[3]);
  await fs.mkdir(output, { recursive: true });
  const server = await serveCactbot(root);
  const kind = process.platform === 'win32' ? 'msedge' : 'chromium';
  const report = { platform: process.platform, kind, scenarios: [], errors: [] };
  let browser;
  try {
    browser = await chromium.launch({
      channel: kind === 'msedge' ? 'msedge' : undefined,
      headless: true, args: process.platform === 'linux' ? ['--enable-speech-dispatcher'] : [],
      // Playwright adds --mute-audio by default in headless Chromium; remove it for an actual output test.
      ignoreDefaultArgs: ['--mute-audio'],
    });
    report.browser = await browser.version();
    const probe = await browser.newPage();
    await probe.evaluate(() => window.speechSynthesis?.getVoices());
    try {
      await probe.waitForFunction(() => window.speechSynthesis?.getVoices().some(voice => voice.localService), null, { timeout: 15000 });
    } catch (error) {
      throw new Error('No local Web Speech voice became ready; install a local voice and enable Chromium Speech Dispatcher on Linux', { cause: error });
    }
    const voices = await probe.evaluate(() => window.speechSynthesis.getVoices().map(voice => ({ name: voice.name, lang: voice.lang, local: voice.localService })));
    report.voiceCount = voices.length;
    const supported = { en: 'en-US', ko: 'ko-KR', de: 'de-DE', fr: 'fr-FR', ja: 'ja-JP', cn: 'zh-CN' };
    const language = process.env.AFTERCAST_AUDIO_LANG || Object.keys(supported).find(lang => voices.some(voice => voice.local && voice.lang.replaceAll('_', '-') === supported[lang]));
    assert(language, 'Install a local voice matching a supported cactbot display language');
    report.language = language;
    report.matchingVoices = voices.filter(voice => voice.local && voice.lang.replaceAll('_', '-') === supported[language]).slice(0, 10);
    assert(report.matchingVoices.length > 0, `No local browser voice matches cactbot display language '${language}'`);
    await probe.close();
    const base = `http://127.0.0.1:${server.address().port}`;
    for (const speech of [false, true]) {
      const page = await browser.newPage();
      page.on('pageerror', error => report.errors.push(error.message));
      page.on('console', message => { if (message.type() === 'error') report.errors.push(message.text()); });
      await page.route('**/*', route => {
        if (new URL(route.request().url()).origin === base) return route.continue();
        report.errors.push('Unexpected external request: ' + route.request().url());
        return route.abort();
      });
      await installFixtureApi(page, language);
      await page.addInitScript(observeAudio);
      await page.goto(base + `/ui/raidboss/raidboss.html?timeline=1&alerts=1&audio=1&remote=1&player=Fixture%20Player${speech ? '&forceTTS=1' : ''}`, { waitUntil: 'networkidle' });
      await page.waitForFunction(() => window.fixtureRequests.some(request => request.call === 'cactbotRequestState'));
      // Preserve normal autoplay restrictions and grant a real browser user activation before firing the log.
      const unlock = page.locator('.autoplay-helper-button');
      const unlockPrompt = (await unlock.count()) > 0;
      if (unlockPrompt) await unlock.click();
      else await page.locator('body').click({ position: { x: 5, y: 5 } });
      await page.waitForFunction(lang => window.speechSynthesis?.getVoices().some(voice => voice.localService && voice.lang.replaceAll('_', '-') === lang), supported[language]);
      await triggerResponse(page);
      const scenario = { speech, unlockPrompt };
      report.scenarios.push(scenario);
      try {
        await page.waitForFunction(speech => window.audioEvents.some(event => event.type === (speech ? 'speech' : 'media') && event.ended), speech, { timeout: 15000 });
        // Repeat the shipped trigger: BrowserTTSEngine reuses its cached utterance object for the same text.
        await triggerResponse(page);
        await page.waitForFunction(speech => {
          const events = window.audioEvents.filter(event => event.type === (speech ? 'speech' : 'media'));
          return events.length >= 2 && events.every(event => event.ended);
        }, speech, { timeout: 15000 });
        if (!speech) {
          for (const [line, asset, volume] of [
            ['You poke the striking dummy.', 'freesound/percussion_hit.webm', 1],
            ['You psych yourself up alongside the striking dummy.', 'BigWigs/Alert.webm', 1],
            ['You clap for the striking dummy.', 'freesound/power_up.webm', 0.3],
          ]) {
            await triggerResponse(page, line, '001D');
            await page.waitForFunction(({ asset, volume }) => window.audioEvents.some(event =>
              event.type === 'media' && event.src.endsWith('/' + asset) && event.playing && event.ended && event.volume === volume),
            { asset, volume }, { timeout: 15000 });
          }
        }
      } finally {
        scenario.events = await page.evaluate(() => window.audioEvents);
        scenario.requests = await page.evaluate(() => window.fixtureRequests);
      }
      assert(!scenario.requests.some(request => ['cactbotSay', 'playSound'].includes(request.call)), 'Browser audio unexpectedly used ACT routing');
      assert(!scenario.events.some(event => event.error || event.type === 'rejection'), 'Browser playback reported a failure');
      if (speech) {
        const spoken = scenario.events.filter(event => event.type === 'speech');
        assert(spoken.every(event => event.text === '4' && event.started && event.ended && event.local && event.volume > 0), 'Shipped response did not finish with a local browser voice');
        assert(!scenario.events.some(event => event.type === 'media'), 'TTS should take priority over effects');
      } else {
        const media = scenario.events.filter(event => event.type === 'media');
        assert(media.every(event => event.volume > 0 && event.muted === false), 'HTMLAudio was muted');
        assert(media.some(event => event.src.endsWith('/BigWigs/Alarm.webm') && event.playing && event.ended && event.time > 0), 'Shipped alarm did not finish native playback');
      }
      await page.close();
    }
    assert.deepEqual(report.errors, []);
    report.passed = true;
  } catch (error) {
    report.failure = String(error.stack || error);
    throw error;
  } finally {
    await fs.writeFile(path.join(output, 'audio-report.json'), JSON.stringify(report, null, 2));
    console.log(JSON.stringify(report, null, 2));
    await browser?.close(); server.closeAllConnections(); server.close();
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
