import { test, expect, chromium, Browser, Page } from '@playwright/test';
import { spawn, ChildProcess } from 'node:child_process';
import { promises as fs } from 'node:fs';
import path from 'node:path';
import net from 'node:net';

const target = path.resolve(__dirname, '../../../../target');
const binary = process.env.AFTERCAST_GUI_EXE || path.join(target, 'debug/aftercast.exe');
let directory: string;
let app: ChildProcess;
let browser: Browser;
let page: Page;
let output = '';

async function start() {
  const server = net.createServer();
  await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  if (!address || typeof address === 'string') throw new Error('No test port');
  const port = address.port;
  await new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
  app = spawn(path.join(directory, 'aftercast.exe'), [], {
    cwd: target,
    env: {
      ...process.env,
      DOTNET_ROOT: path.join(directory, 'no-global-dotnet'),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
      WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview-test'),
    },
  });
  app.stdout?.on('data', data => output += data);
  app.stderr?.on('data', data => output += data);
  await expect.poll(async () => {
    if (app.exitCode !== null) throw new Error(`Application exited: ${output}`);
    try {
      browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 700 });
      return true;
    } catch { return false; }
  }, { timeout: 30_000 }).toBe(true);
  await expect.poll(() => browser.contexts()[0]?.pages().length || 0).toBeGreaterThan(0);
  page = browser.contexts()[0].pages()[0];
  await expect(page.locator('.app-shell')).toBeVisible();
}

async function quit() {
  await page.locator('.topbar button').last().click();
  await expect.poll(() => app.exitCode, { timeout: 15_000 }).not.toBeNull();
  expect(app.exitCode, output).toBe(0);
}

test.beforeEach(async () => {
  directory = await fs.mkdtemp(path.join(target, 'gui-e2e-'));
  await fs.copyFile(binary, path.join(directory, 'aftercast.exe'));
  output = '';
});

test.afterEach(async ({}, info) => {
  if (info.status !== info.expectedStatus) {
    if (page && !page.isClosed()) await page.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
    await fs.writeFile(path.join(directory, 'process-output.txt'), output);
  }
  if (app && app.exitCode === null && app.pid) {
    await new Promise<void>(resolve => {
      spawn('taskkill', ['/PID', String(app.pid), '/T', '/F'], { windowsHide: true }).on('close', () => resolve());
    });
  }
  if (info.status === info.expectedStatus) {
    // Delete only this test's generated workspace directory after its owned process has stopped.
    const resolved = path.resolve(directory);
    if (!resolved.startsWith(target + path.sep + 'gui-e2e-')) throw new Error('Unsafe test cleanup path');
    await fs.rm(resolved, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  } else console.log(`Failure artifacts: ${directory}`);
});

test('locale, theme and annotated TOML survive restart beside the executable', async () => {
  await fs.writeFile(path.join(directory, 'config.toml'), '# preserve this note\nschema_version = 1\n[ui]\nlanguage = "ko-KR"\ntheme = "mocha"\ncustom = 42\n');
  await start();
  await page.getByRole('button', { name: '설정', exact: true }).click();
  await page.locator('#language').selectOption('en-US');
  await expect(page.getByRole('heading', { name: 'Settings', exact: true })).toBeVisible();
  await page.locator('#theme').selectOption('latte');
  await page.getByRole('button', { name: 'Save settings', exact: true }).click();
  await expect(page.getByRole('status').last()).toHaveText('Settings saved');
  const saved = await fs.readFile(path.join(directory, 'config.toml'), 'utf8');
  expect(saved).toContain('# preserve this note');
  expect(saved).toContain('custom = 42');
  expect(saved).toContain('language = "en-US"');
  await page.screenshot({ path: path.join(target, 'gui-settings-light.png') });
  await quit();
  await start();
  await expect(page.locator('.app-shell')).toHaveAttribute('data-theme', 'latte');
  await expect(page.getByRole('heading', { name: 'Analysis', exact: true })).toBeVisible();
  await quit();
});

test('invalid configuration is reported and never overwritten', async () => {
  const original = 'schema_version = 99\n';
  await fs.writeFile(path.join(directory, 'config.toml'), original);
  await start();
  await expect(page.getByRole('alert')).toContainText('schema_version');
  await page.getByRole('button', { name: '설정', exact: true }).click();
  await expect(page.getByRole('button', { name: '설정 저장', exact: true })).toBeDisabled();
  expect(await fs.readFile(path.join(directory, 'config.toml'), 'utf8')).toBe(original);
  await quit();
});

test('settings save and reload survive navigation away from their pane', async () => {
  await fs.writeFile(path.join(directory, 'config.toml'), '[ui]\nlanguage = "en-US"\n');
  await start();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.locator('#theme').selectOption('latte');
  // Dispatch both actions together to cover unmounting while the file operation is pending.
  await page.evaluate(() => {
    const save = document.querySelector<HTMLButtonElement>('#save-settings')!;
    const analysis = [...document.querySelectorAll<HTMLButtonElement>('nav button')].find(button => button.textContent?.includes('Analysis'))!;
    save.click();
    analysis.click();
  });
  await expect(page.getByRole('heading', { name: 'Analysis', exact: true })).toBeVisible();
  await expect(page.getByRole('status').last()).toHaveText('Settings saved');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.locator('#save-settings')).toBeEnabled();
  const config = await fs.readFile(path.join(directory, 'config.toml'), 'utf8');
  expect(config).toContain('theme = "latte"');
  await fs.writeFile(path.join(directory, 'config.toml'), config.replace('theme = "latte"', 'theme = "mocha"'));
  await page.evaluate(() => {
    const reload = [...document.querySelectorAll<HTMLButtonElement>('button')].find(button => button.textContent?.includes('Reload settings'))!;
    const analysis = [...document.querySelectorAll<HTMLButtonElement>('nav button')].find(button => button.textContent?.includes('Analysis'))!;
    reload.click();
    analysis.click();
  });
  await expect(page.getByRole('heading', { name: 'Analysis', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.locator('#theme')).toHaveValue('mocha');
  await expect(page.locator('#save-settings')).toBeEnabled();
  await quit();
});

test('embedded runtime starts a real Host, imports expected totals and survives restart', async () => {
  const plugin = path.resolve(target, '../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll');
  await fs.access(plugin);
  await fs.writeFile(path.join(directory, 'config.toml'), `schema_version = 1\n[ui]\nlanguage = "en-US"\n[host]\nplugins = [${JSON.stringify(plugin)}]\n[host.plugin_types]\n${JSON.stringify(plugin)} = "Aftercast.PluginFixture.ImportPlugin"\n`);
  const log = path.join(directory, 'fixture.log');
  await fs.writeFile(log, '0|100\n2|300\n');
  await start();
  await page.getByRole('button', { name: 'Start host', exact: true }).click();
  await expect(page.getByTestId('host-status')).toHaveText('Host running', { timeout: 40_000 });
  await page.getByRole('button', { name: 'Analysis', exact: true }).click();
  await page.locator('#log-path').fill(log);
  await page.getByRole('button', { name: 'Import log', exact: true }).click();
  await expect(page.getByRole('status').last()).toContainText('Import completed', { timeout: 30_000 });
  await expect(page.locator('.encounter-item')).toHaveCount(1);
  await page.locator('.encounter-item').click();
  await expect(page.locator('.summary-strip')).toContainText('400');
  await expect(page.locator('.data-table')).toContainText('400');
  await expect(page.getByRole('alert')).toHaveCount(0);
  await page.screenshot({ path: path.join(target, 'gui-fixture-dark.png') });
  await page.setViewportSize({ width: 850, height: 580 });
  await expect(page.locator('.app-shell')).toBeVisible();
  await page.screenshot({ path: path.join(target, 'gui-fixture-compact.png') });
  await page.getByRole('button', { name: 'Stop host', exact: true }).click();
  await page.setViewportSize({ width: 1280, height: 820 });
  await expect(page.getByTestId('host-status')).toHaveText('Host stopped');
  const caches = (await fs.readdir(path.join(directory, 'dotnet'), { withFileTypes: true })).filter(entry => entry.isDirectory() && !entry.name.startsWith('.'));
  expect(caches).toHaveLength(1);
  const marker = path.join(directory, 'dotnet', caches[0].name, '.complete.toml');
  const timestamp = (await fs.stat(marker)).mtimeMs;
  await page.getByRole('button', { name: 'Start host', exact: true }).click();
  await expect(page.getByTestId('host-status')).toHaveText('Host running', { timeout: 40_000 });
  expect((await fs.stat(marker)).mtimeMs).toBe(timestamp);
  await page.getByRole('button', { name: 'Stop host', exact: true }).click();
  await expect(page.getByTestId('host-status')).toHaveText('Host stopped');
  await quit();
});
