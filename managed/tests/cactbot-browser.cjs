const fs = require('node:fs/promises');
const path = require('node:path');
const http = require('node:http');
const assert = require('node:assert/strict');
const { chromium } = require('playwright');

// Exercise unmodified release pages with explicit fixture responses, independently of ACT/CEF startup.
async function main() {
  const root = path.resolve(process.argv[2]);
  const output = path.resolve(process.argv[3]);
  await fs.mkdir(output, { recursive: true });
  const server = http.createServer(async (req, res) => {
    const file = path.resolve(root, '.' + decodeURIComponent(new URL(req.url, 'http://localhost').pathname));
    if (!file.startsWith(root + path.sep)) { res.writeHead(403).end(); return; }
    try {
      const data = await fs.readFile(file);
      const types = { '.js': 'text/javascript', '.html': 'text/html', '.css': 'text/css', '.png': 'image/png', '.mp3': 'audio/mpeg' };
      res.setHeader('Content-Type', types[path.extname(file)] || 'application/octet-stream');
      res.end(data);
    } catch { res.writeHead(404).end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  let browser;
  const errors = [];
  try {
    browser = await chromium.launch({ channel: 'msedge', headless: true, args: ['--mute-audio'] });
    const page = await browser.newPage();
    page.on('pageerror', error => errors.push(error.message));
    await page.addInitScript(() => {
      window.fixtureRequests = [];
      window.OverlayPluginApi = { ready: true, callHandler: (json, callback) => {
        const request = JSON.parse(json);
        window.fixtureRequests.push(request);
        let response = {};
        if (request.call === 'cactbotLoadUser') response = { detail: {
          userLocation: 'fixture', localUserFiles: {}, parserLanguage: 'en', systemLocale: 'en-US', displayLanguage: 'en',
          cactbotVersion: '0.37.5', overlayPluginVersion: '0.19.111', ffxivPluginVersion: '3.0.3.4', actVersion: '3.8.5.288', gameRegion: 'Global',
        } };
        else if (request.call === 'cactbotLoadData') response = { data: {} };
        else if (request.call === 'getCombatants') response = { combatants: [] };
        callback?.(JSON.stringify(response));
      } };
    });
    const base = `http://127.0.0.1:${server.address().port}`;
    await page.goto(base + '/ui/raidboss/raidboss.html?timeline=1&alerts=1&audio=0', { waitUntil: 'networkidle' });
    await page.waitForFunction(() => window.fixtureRequests.some(r => r.call === 'cactbotRequestState'));
    await page.evaluate(() => {
      window.__OverlayCallback({ type: 'onPlayerChangedEvent', detail: { name: 'Fixture Player', id: 0x10000001, job: 'PLD', level: 100, currentHP: 1000, maxHP: 1000 } });
      window.__OverlayCallback({ type: 'ChangeZone', zoneID: 134, zoneName: 'Middle La Noscea' });
      window.__OverlayCallback({ type: 'onInCombatChangedEvent', detail: { inGameCombat: true } });
      const rawLine = '00|2026-01-01T00:00:00.0000000+00:00|0038||cactbot test response|fixture';
      window.__OverlayCallback({ type: 'LogLine', rawLine, line: rawLine.split('|') });
    });
    await page.waitForFunction(() => document.querySelector('#popup-text-alarm .holder')?.textContent.includes('1'));
    const popups = await page.locator('#popup-text-container .holder').allTextContents();
    assert(popups.some(text => text.includes('1')) && popups.some(text => text.includes('2')) && popups.some(text => text.includes('3')));
    await page.evaluate(() => {
      const rawLine = '00|2026-01-01T00:00:01.0000000+00:00|001D||You bow courteously to the striking dummy.|fixture';
      window.__OverlayCallback({ type: 'LogLine', rawLine, line: rawLine.split('|') });
    });
    await page.waitForFunction(() => document.querySelectorAll('#timeline timer-bar').length > 0);
    const timeline = await page.locator('#timeline timer-bar').evaluateAll(bars => bars.map(bar => bar.getAttribute('lefttext')).join('\n'));
    assert(timeline.includes('Angry Dummy'));
    const requests = await page.evaluate(() => window.fixtureRequests);
    const allowed = new Set(['subscribe', 'cactbotLoadUser', 'cactbotLoadData', 'cactbotRequestState']);
    assert(requests.every(request => allowed.has(request.call)), 'Unspecified fixture handler response');
    await page.screenshot({ path: path.join(output, 'raidboss.png') });
    await page.evaluate(() => {
      const rawLine = '00|2026-01-01T00:00:02.0000000+00:00|0038||test sync2|fixture';
      window.__OverlayCallback({ type: 'LogLine', rawLine, line: rawLine.split('|') });
    });
    await page.waitForFunction(() => [...document.querySelectorAll('#timeline timer-bar')].some(bar => bar.getAttribute('lefttext') === 'Three'));
    const jumpedTimeline = await page.locator('#timeline timer-bar').evaluateAll(bars => bars.map(bar => bar.getAttribute('lefttext')).join('\n'));
    await page.evaluate(() => {
      const rawLine = '00|2026-01-01T00:00:03.0000000+00:00|001D||You bid farewell to the striking dummy.|fixture';
      window.__OverlayCallback({ type: 'LogLine', rawLine, line: rawLine.split('|') });
    });
    await page.waitForFunction(() => document.querySelectorAll('#timeline timer-bar').length === 0);
    assert.deepEqual(errors, []);
    const report = { browser: await browser.version(), popups, timeline, jumpedTimeline, reset: true, requests, errors,
      scope: 'Unmodified cactbot web bundles with fixture OverlayPlugin API; not an aftercast/OverlayPlugin/CEF integration pass' };
    await fs.writeFile(path.join(output, 'browser-report.json'), JSON.stringify(report, null, 2));
    console.log(JSON.stringify(report, null, 2));
  } finally { await browser?.close(); server.closeAllConnections(); server.close(); }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
