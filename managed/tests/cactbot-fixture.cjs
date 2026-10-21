const fs = require('node:fs/promises');
const path = require('node:path');
const http = require('node:http');

// Serve the unchanged release tree, including Chromium's WebM sound assets.
async function serveCactbot(root) {
  const server = http.createServer(async (req, res) => {
    try {
      if (req.url === '/favicon.ico') { res.writeHead(204).end(); return; }
      const file = path.resolve(root, '.' + decodeURIComponent(new URL(req.url, 'http://localhost').pathname));
      if (!file.startsWith(root + path.sep)) { res.writeHead(403).end(); return; }
      const data = await fs.readFile(file);
      const types = { '.js': 'text/javascript', '.html': 'text/html', '.css': 'text/css', '.png': 'image/png', '.mp3': 'audio/mpeg', '.webm': 'audio/webm' };
      res.setHeader('Content-Type', types[path.extname(file)] || 'application/octet-stream');
      res.end(data);
    } catch { res.writeHead(404).end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  return server;
}

async function installFixtureApi(page, displayLanguage = 'en') {
  await page.addInitScript(displayLanguage => {
    window.fixtureRequests = [];
    window.OverlayPluginApi = { ready: true, callHandler: (json, callback) => {
      const request = JSON.parse(json);
      window.fixtureRequests.push(request);
      let response = {};
      if (request.call === 'cactbotLoadUser') response = { detail: {
        userLocation: 'fixture', localUserFiles: {}, parserLanguage: 'en', systemLocale: 'en-US', displayLanguage,
        cactbotVersion: '0.37.5', overlayPluginVersion: '0.19.111', ffxivPluginVersion: '3.0.3.4', actVersion: '3.8.5.288', gameRegion: 'Global',
      } };
      else if (request.call === 'cactbotLoadData') response = { data: {} };
      else if (request.call === 'getCombatants') response = { combatants: [] };
      callback?.(JSON.stringify(response));
    } };
  }, displayLanguage);
}

// Use the shipped Summerford response trigger instead of synthesizing a sound/speech API call.
async function triggerResponse(page) {
  await page.evaluate(() => {
    window.__OverlayCallback({ type: 'onPlayerChangedEvent', detail: { name: 'Fixture Player', id: 0x10000001, job: 'PLD', level: 100, currentHP: 1000, maxHP: 1000 } });
    window.__OverlayCallback({ type: 'ChangeZone', zoneID: 134, zoneName: 'Middle La Noscea' });
    const rawLine = '00|2026-01-01T00:00:00.0000000+00:00|0038||cactbot test response|fixture';
    window.__OverlayCallback({ type: 'LogLine', rawLine, line: rawLine.split('|') });
  });
}

module.exports = { serveCactbot, installFixtureApi, triggerResponse };
