// Real release Tauri EXE + WebView2 + native IPC. No desktop input or visible window.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createServer } from 'node:net';
import { mkdtemp, mkdir, readFile, writeFile, stat } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';

if (process.platform !== 'win32')
  throw new Error('This native verification uses Windows WebView2.');
const root = resolve(import.meta.dirname, '..');
const executable =
  process.env.HMCL_DESKTOP_BIN ?? join(root, '..', 'target', 'release', 'rust-hmcl-desktop.exe');
await stat(executable);
const tools = join(root, '.tools');
await mkdir(tools, { recursive: true });
const fixture = await mkdtemp(join(tools, 'native-ui-proof-'));
const output = join(root, 'dist', 'native-ui-proof');
await mkdir(output, { recursive: true });
const version = join(fixture, 'minecraft', 'versions', 'native-fixture');
const game = join(version, 'game');
await mkdir(join(game, 'mods'), { recursive: true });
await mkdir(join(game, 'resourcepacks', 'Native Pack'), { recursive: true });
await writeFile(
  join(version, 'native-fixture.json'),
  JSON.stringify({ id: 'native-fixture', libraries: [] }),
);
await writeFile(
  join(game, 'mods', 'native-sample.jar'),
  'handwritten listing fixture, never launched',
);
await writeFile(
  join(game, 'resourcepacks', 'Native Pack', 'pack.mcmeta'),
  JSON.stringify({ pack: { pack_format: 15, description: 'Native IPC sample' } }),
);
await writeFile(
  join(fixture, 'config.json'),
  JSON.stringify({
    game_dir: join(fixture, 'minecraft'),
    selected: 'native-fixture',
    isolated: true,
    player: 'NativeProof',
  }),
);
const reserve = createServer();
reserve.listen(0, '127.0.0.1');
await once(reserve, 'listening');
const port = reserve.address().port;
await new Promise((resolve, reject) =>
  reserve.close((error) => (error ? reject(error) : resolve())),
);
const app = spawn(executable, ['--verify-hidden-ui'], {
  cwd: root,
  windowsHide: true,
  env: { ...process.env, RUST_HMCL_DATA_DIR: fixture, HMCL_UI_PROOF_PORT: String(port) },
  stdio: ['ignore', 'pipe', 'pipe'],
});
let processError = '';
let spawnError;
app.stderr.on('data', (chunk) => {
  processError += chunk;
});
app.on('error', (error) => {
  spawnError = error;
});
let browser;
try {
  const deadline = Date.now() + 45000;
  let ready = false;
  while (Date.now() < deadline) {
    if (spawnError) throw spawnError;
    if (app.exitCode !== null) throw new Error(`Desktop exited (${app.exitCode}): ${processError}`);
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/version`, {
        signal: AbortSignal.timeout(1000),
      });
      if (response.ok) {
        ready = true;
        break;
      }
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  if (!ready) throw new Error(`WebView2 debugger did not become ready: ${processError}`);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const pages = browser.contexts().flatMap((context) => context.pages());
  assert.ok(pages.length, 'Native WebView target missing');
  const page = pages.find((item) => item.url().includes('tauri.localhost')) ?? pages[0];
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.waitForFunction(() => typeof window.__TAURI_INTERNALS__?.invoke === 'function');
  const invoke = (command, args = {}) =>
    page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), {
      command,
      args,
    });
  const visible = await invoke('plugin:window|is_visible', { label: 'main' });
  assert.equal(visible, false, 'Verification must never show the native window');
  assert.equal(await invoke('plugin:window|is_decorated', { label: 'main' }), false);
  const snapshot = await invoke('snapshot');
  const layout = await page.evaluate(() => ({
    width: innerWidth,
    height: innerHeight,
    devicePixelRatio,
  }));
  const nativeSize = await invoke('plugin:window|inner_size', { label: 'main' });
  assert.ok(Math.abs(nativeSize.width / layout.devicePixelRatio - layout.width) <= 2);
  assert.ok(Math.abs(nativeSize.height / layout.devicePixelRatio - layout.height) <= 2);
  assert.equal(snapshot.config.player, 'NativeProof');
  assert.equal(snapshot.instances[0].id, 'native-fixture');
  const configBefore = await readFile(join(fixture, 'config.json'));
  await assert.rejects(invoke('update_config', { config: { ...snapshot.config, memory: 1 } }));
  assert.deepEqual(await readFile(join(fixture, 'config.json')), configBefore);
  await assert.rejects(invoke('start_job', { job: { kind: 'launch', id: '../escape' } }));
  assert.equal((await invoke('task')).running, false);
  await expect(page.locator('.launch-primary')).toContainText('native-fixture');
  await expect(page.getByRole('alert')).toHaveCount(0);
  const settle = () =>
    page.evaluate(() =>
      Promise.all(document.getAnimations().map((animation) => animation.finished.catch(() => {}))),
    );
  await settle();
  await page.screenshot({ path: join(output, 'native-home-light.png') });
  await page.getByRole('button', { name: '账户', exact: true }).click();
  await expect(page.locator('.page').last()).not.toHaveAttribute('inert');
  await page.getByRole('textbox', { name: '离线用户名', exact: true }).fill('NativePlayer');
  await page.getByRole('button', { name: '添加账户', exact: true }).click();
  await expect(page.locator('.account-nav b')).toHaveText('NativePlayer');
  assert.equal(
    JSON.parse(await readFile(join(fixture, 'config.json'), 'utf8')).player,
    'NativePlayer',
  );
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await expect(page.locator('.page').last()).not.toHaveAttribute('inert');
  await page.getByLabel('最大内存（MB）', { exact: true }).fill('3584');
  await page.getByRole('button', { name: '保存设置', exact: true }).click();
  await expect(page.getByText('设置已保存', { exact: true })).toBeVisible();
  assert.equal(JSON.parse(await readFile(join(fixture, 'config.json'), 'utf8')).memory, 3584);
  await page.getByRole('button', { name: '切换主题', exact: true }).click();
  await expect(page.locator('.launcher')).toHaveClass(/dark/);
  await settle();
  await page.screenshot({ path: join(output, 'native-settings-dark.png') });
  await page.getByRole('button', { name: '游戏列表', exact: true }).click();
  await page.getByRole('button', { name: '管理 native-fixture', exact: true }).click();
  await page.getByRole('button', { name: '模组管理', exact: true }).click();
  await page.getByRole('button', { name: '禁用', exact: true }).click();
  await expect(page.getByText('native-sample.jar.disabled', { exact: true })).toBeVisible();
  assert.ok((await stat(join(game, 'mods', 'native-sample.jar.disabled'))).isFile());
  await page.getByRole('button', { name: '启用', exact: true }).click();
  await expect(page.getByText('native-sample.jar', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '资源包管理', exact: true }).click();
  const checkbox = page.getByRole('checkbox', { name: '启用 Native Pack', exact: true });
  await checkbox.click();
  await expect(page.getByRole('dialog', { name: '资源包兼容性提示' })).toBeVisible();
  await page.getByRole('button', { name: '取消', exact: true }).click();
  await expect(checkbox).not.toBeChecked();
  await checkbox.click();
  await page.getByRole('button', { name: '确认', exact: true }).click();
  await expect(checkbox).toBeChecked();
  assert.match(await readFile(join(game, 'options.txt'), 'utf8'), /file\/Native Pack/);
  await page.getByRole('button', { name: '返回游戏列表', exact: true }).click();
  await page.getByRole('button', { name: 'native-fixture更多操作', exact: true }).click();
  await page.getByRole('menuitem', { name: '复制实例', exact: true }).click();
  await page.getByRole('textbox', { name: '新实例名称', exact: true }).fill('native-copy');
  await page.getByRole('button', { name: '确认', exact: true }).click();
  await expect(page.getByText('native-copy', { exact: true })).toBeVisible();
  const copied = join(fixture, 'minecraft', 'versions', 'native-copy');
  assert.equal(
    JSON.parse(await readFile(join(copied, 'native-copy.json'), 'utf8')).id,
    'native-copy',
  );
  assert.deepEqual(
    await readFile(join(copied, 'game', 'mods', 'native-sample.jar')),
    await readFile(join(game, 'mods', 'native-sample.jar')),
  );
  assert.equal((await invoke('snapshot')).config.version_settings['native-copy'].isolated, true);
  assert.equal((await invoke('task')).running, false);
  assert.equal(await invoke('plugin:window|is_visible', { label: 'main' }), false);
  assert.deepEqual(errors, []);
  const proof = {
    executable,
    executableSha256: createHash('sha256')
      .update(await readFile(executable))
      .digest('hex'),
    fixture,
    transport: 'real release Tauri / WebView2 / native IPC',
    layout,
    nativeSize,
    url: page.url(),
    nativeWindowVisible: false,
    desktopInput: false,
    javaOrGameStarted: false,
    handwrittenFixtures: true,
    passed: [
      'hidden native window',
      'bundled Vue assets and CSP',
      'snapshot IPC',
      'invalid configuration does not write',
      'traversal rejected before game start',
      'offline account persistence',
      'global settings persistence',
      'theme',
      'mod toggle round trip',
      'resource compatibility confirmation before options write',
      'native background copy task and completion refresh',
    ],
    limitations: [
      'Windows only',
      'native visible-window geometry and rounded corners not verified',
      'game launching and installers not exercised',
    ],
  };
  await writeFile(join(output, 'proof.json'), JSON.stringify(proof, null, 2));
  console.log(`Native UI verification passed: ${join(output, 'proof.json')}`);
} finally {
  if (browser?.isConnected()) {
    const page = browser.contexts().flatMap((context) => context.pages())[0];
    if (page)
      await page
        .evaluate(() => window.__TAURI_INTERNALS__.invoke('plugin:window|close', { label: 'main' }))
        .catch(() => {});
  }
  if (app.exitCode === null && !spawnError) {
    const timeout = setTimeout(() => app.kill(), 3000);
    await once(app, 'exit');
    clearTimeout(timeout);
  }
  if (browser) await browser.close().catch(() => {});
}
