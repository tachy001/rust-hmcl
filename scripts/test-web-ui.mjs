// Real built frontend -> JSON-lines harness -> shared Rust Service.
// Uses a headless browser and independent handwritten fixtures, never the desktop.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createInterface } from 'node:readline';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, writeFile, stat } from 'node:fs/promises';
import { resolve, join, sep, extname } from 'node:path';
import { emphasized } from '../frontend/src/motion.ts';
import assert from 'node:assert/strict';

const root = resolve(import.meta.dirname, '..');
const output = join(root, 'dist', 'web-ui-proof');
await mkdir(output, { recursive: true });
const fixture = await mkdtemp(join(root, '..', '.tools', 'web-ui-proof-'));
const version = join(fixture, 'minecraft', 'versions', '1.20.1');
const game = join(version, 'game');
await mkdir(join(game, 'resourcepacks', 'Demo Pack'), { recursive: true });
await mkdir(join(game, 'mods'), { recursive: true });
await writeFile(join(version, '1.20.1.json'), JSON.stringify({ id: '1.20.1', libraries: [] }));
await writeFile(
  join(game, 'resourcepacks', 'Demo Pack', 'pack.mcmeta'),
  JSON.stringify({
    pack: {
      pack_format: 15,
      description: '§aDemo description§r\nSecond line'.replace('\\n', '\n'),
    },
  }),
);
await writeFile(join(game, 'mods', 'sample.jar'), 'handwritten listing fixture; never launched');
await writeFile(
  join(fixture, 'config.json'),
  JSON.stringify({
    game_dir: join(fixture, 'minecraft'),
    selected: '1.20.1',
    player: 'PreviewUser',
    isolated: true,
    version_settings: { untouched: { memory: 8192, width: 1280 } },
  }),
);
const executable =
  process.env.HMCL_SERVICE_BIN ??
  join(
    root,
    '..',
    'target',
    'debug',
    process.platform === 'win32' ? 'hmcl-service.exe' : 'hmcl-service',
  );
const backend = spawn(executable, [], {
  cwd: root,
  env: { ...process.env, RUST_HMCL_DATA_DIR: fixture },
  windowsHide: true,
  stdio: ['pipe', 'pipe', 'pipe'],
});
const requests = [];
let backendError = '';
backend.stderr.on('data', (chunk) => {
  backendError += chunk;
});
backend.on('error', (error) => {
  while (requests.length) requests.shift().reject(error);
});
backend.on('exit', (code) => {
  while (requests.length)
    requests.shift().reject(new Error(`Service exited (${code}): ${backendError}`));
});
createInterface({ input: backend.stdout }).on('line', (line) => {
  const request = requests.shift();
  if (!request) return;
  try {
    const response = JSON.parse(line);
    response.ok ? request.resolve(response.value) : request.reject(new Error(response.error));
  } catch (error) {
    request.reject(error);
  }
});
function rpc(method, args = {}) {
  return new Promise((resolve, reject) => {
    if (backend.exitCode !== null) {
      reject(new Error(backendError || 'Backend unavailable'));
      return;
    }
    requests.push({ resolve, reject });
    backend.stdin.write(JSON.stringify({ method, ...args }) + '\n');
  });
}
const publicRoot = join(root, 'web-dist');
const mime = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
};
const server = createServer(async (req, res) => {
  try {
    const pathname = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
    const path = resolve(publicRoot, '.' + (pathname === '/' ? '/index.html' : pathname));
    if (!path.startsWith(publicRoot + sep)) throw Error('outside static root');
    const content = await readFile(path);
    res.writeHead(200, { 'Content-Type': mime[extname(path)] ?? 'application/octet-stream' });
    res.end(content);
  } catch {
    res.writeHead(404);
    res.end();
  }
});
let browser;
try {
  assert.equal(emphasized(0), 0);
  assert.equal(emphasized(1), 1);
  assert.ok(Math.abs(emphasized(0.166666) - 0.4) < 1e-5);
  const initial = await rpc('snapshot');
  assert.equal(initial.instances[0].id, '1.20.1');
  const originalConfig = await readFile(join(fixture, 'config.json'));
  await assert.rejects(rpc('update_config', { config: { ...initial.config, memory: 1 } }));
  assert.deepEqual(await readFile(join(fixture, 'config.json')), originalConfig);
  await assert.rejects(rpc('start_job', { job: { kind: 'launch', id: '../escape' } }));
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const base = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch(
    process.platform === 'win32' ? { channel: 'msedge', headless: true } : { headless: true },
  );
  const context = await browser.newContext({
    viewport: { width: 1080, height: 700 },
    deviceScaleFactor: 1,
    reducedMotion: 'no-preference',
  });
  await context.exposeBinding('__hmclInvoke', async (_source, command, args) => {
    if (command === 'start_job')
      throw Error('Headless UI verification never starts Java, games, downloads or installers.');
    return rpc(command, args ?? {});
  });
  await context.addInitScript(() => {
    window.__TAURI_INTERNALS__ = {
      invoke: (command, args) =>
        window.__hmclInvoke(command, JSON.parse(JSON.stringify(args ?? {}))),
    };
  });
  const page = await context.newPage();
  const failures = [];
  page.on('pageerror', (error) => failures.push(error.message));
  await page.goto(base);
  await expect(page.getByRole('button', { name: '选择游戏版本', exact: true })).toBeEnabled();
  await expect(page.locator('.launch-primary')).toContainText('1.20.1');
  const settle = () =>
    page.evaluate(() =>
      Promise.all(document.getAnimations().map((animation) => animation.finished.catch(() => {}))),
    );
  await settle();
  await page.screenshot({ path: join(output, 'home-light.png') });
  await page.getByRole('button', { name: '选择游戏版本', exact: true }).click();
  await expect(page.getByRole('menu')).toBeVisible();
  await settle();
  await page.screenshot({ path: join(output, 'version-selector.png') });
  await page.getByRole('button', { name: '选择游戏版本', exact: true }).click();
  await expect(page.getByRole('menu')).toHaveCount(0);
  await page.getByRole('button', { name: '账户', exact: true }).click();
  const animation = await page
    .locator('.page')
    .last()
    .evaluate((element) => ({
      inert: element.inert,
      opacity: Number(getComputedStyle(element).opacity),
      duration: getComputedStyle(element).transitionDuration,
    }));
  assert.equal(animation.inert, true);
  assert.equal(animation.duration, '0.4s, 0.4s');
  await expect(page.locator('.page').last()).not.toHaveAttribute('inert');
  await page.getByRole('textbox', { name: '离线用户名' }).fill('RustPlayer');
  await page.getByRole('button', { name: '添加账户', exact: true }).click();
  await expect(page.locator('.account-nav b')).toHaveText('RustPlayer');
  assert.ok(
    JSON.parse(await readFile(join(fixture, 'config.json'), 'utf8')).accounts.includes(
      'RustPlayer',
    ),
  );
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await expect(page.locator('.page').last()).not.toHaveAttribute('inert');
  await page.getByLabel('最大内存（MB）', { exact: true }).fill('3072');
  await expect(page.getByLabel('最大内存（MB）', { exact: true })).toHaveValue('3072');
  await page.getByRole('button', { name: '保存设置', exact: true }).click();
  await expect
    .poll(async () => JSON.parse(await readFile(join(fixture, 'config.json'), 'utf8')).memory)
    .toBe(3072);
  await page.getByRole('button', { name: '切换主题', exact: true }).click();
  await expect(page.locator('.launcher')).toHaveClass(/dark/);
  await page.locator('.page').evaluate((element) => (element.scrollTop = 0));
  await settle();
  await page.screenshot({ path: join(output, 'settings-dark.png') });
  await page.getByRole('button', { name: '游戏列表', exact: true }).click();
  await page.getByRole('button', { name: '管理 1.20.1', exact: true }).click();
  await expect(page.locator('.page').last()).not.toHaveAttribute('inert');
  await page.getByRole('checkbox', { name: '单独设置最大内存（MB）', exact: true }).check();
  await page.getByLabel('最大内存（MB）', { exact: true }).fill('4096');
  await page.getByRole('checkbox', { name: '单独设置窗口模式', exact: true }).check();
  await page.getByLabel('窗口模式', { exact: true }).selectOption('true');
  await page.getByRole('checkbox', { name: '单独设置额外 JVM 参数', exact: true }).check();
  await page.getByLabel('额外 JVM 参数', { exact: true }).fill('-Dinstance=true');
  await page.getByRole('button', { name: '切换主题', exact: true }).click();
  await expect(page.locator('.launcher')).not.toHaveClass(/dark/);
  await page.getByRole('button', { name: '切换主题', exact: true }).click();
  await expect(page.locator('.launcher')).toHaveClass(/dark/);
  await expect(page.getByLabel('最大内存（MB）', { exact: true })).toHaveValue('4096');
  await page.getByRole('button', { name: '保存实例设置', exact: true }).click();
  await expect(page.getByText('实例设置已保存', { exact: true })).toBeVisible();
  let stored = JSON.parse(await readFile(join(fixture, 'config.json'), 'utf8'));
  assert.equal(stored.memory, 3072);
  assert.equal(stored.version_settings.untouched.memory, 8192);
  assert.equal(stored.version_settings['1.20.1'].memory, 4096);
  assert.equal(stored.version_settings['1.20.1'].fullscreen, true);
  assert.equal(stored.version_settings['1.20.1'].extra_jvm, '-Dinstance=true');
  await page.locator('.page').evaluate((element) => (element.scrollTop = 0));
  await settle();
  await page.screenshot({ path: join(output, 'instance-settings-dark.png') });
  await page.getByRole('button', { name: '恢复全局设置', exact: true }).click();
  assert.equal(
    JSON.parse(await readFile(join(fixture, 'config.json'), 'utf8')).version_settings['1.20.1']
      .memory,
    4096,
  );
  await page.getByRole('button', { name: '保存实例设置', exact: true }).click();
  await expect
    .poll(
      async () =>
        JSON.parse(await readFile(join(fixture, 'config.json'), 'utf8')).version_settings['1.20.1'],
    )
    .toBeUndefined();
  stored = JSON.parse(await readFile(join(fixture, 'config.json'), 'utf8'));
  assert.equal(stored.memory, 3072);
  assert.equal(stored.version_settings.untouched.width, 1280);
  await page.getByRole('button', { name: '模组管理', exact: true }).click();
  await page.getByRole('button', { name: '禁用', exact: true }).click();
  await expect(page.getByText('sample.jar.disabled', { exact: true })).toBeVisible();
  assert.ok((await stat(join(game, 'mods', 'sample.jar.disabled'))).isFile());
  await page.getByRole('button', { name: '启用', exact: true }).click();
  await expect(page.getByText('sample.jar', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '资源包管理', exact: true }).click();
  const checkbox = page.getByRole('checkbox', { name: '启用 Demo Pack', exact: true });
  await checkbox.click();
  await expect(page.getByRole('dialog', { name: '资源包兼容性提示' })).toBeVisible();
  await expect(checkbox).not.toBeChecked();
  await expect(page.getByRole('button', { name: '取消', exact: true })).toBeFocused();
  await page.getByRole('button', { name: '取消', exact: true }).click();
  await expect(checkbox).not.toBeChecked();
  await checkbox.click();
  await page.getByRole('button', { name: '确认', exact: true }).click();
  await expect(checkbox).toBeChecked();
  assert.match(await readFile(join(game, 'options.txt'), 'utf8'), /file\/Demo Pack/);
  await page.setViewportSize({ width: 800, height: 560 });
  await page.screenshot({ path: join(output, 'resource-dark-narrow.png') });
  assert.equal(
    await page.locator('.page').evaluate((element) => element.scrollWidth <= element.clientWidth),
    true,
  );
  await page.getByRole('button', { name: 'Demo Pack详情', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Demo Pack', exact: true })).toBeVisible();
  await page.screenshot({ path: join(output, 'resource-info-dark.png') });
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.getByRole('button', { name: '返回首页', exact: true }).click();
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await expect(page.locator('.page').last()).not.toHaveAttribute('inert');
  await page.getByRole('checkbox', { name: '减少动画和动态效果', exact: true }).check();
  await page.getByRole('button', { name: '保存设置', exact: true }).click();
  await expect(page.locator('.launcher')).toHaveClass(/reduced/);
  await page.getByRole('button', { name: '游戏列表', exact: true }).click();
  await expect(page.locator('.page').last()).not.toHaveAttribute('inert');
  assert.equal(
    await page
      .locator('.page')
      .last()
      .evaluate((element) => getComputedStyle(element).transitionDuration),
    '0s',
  );
  await rpc('start_job', { job: { kind: 'duplicate', id: 'missing-fixture', name: 'copy' } });
  await expect
    .poll(async () => {
      const task = await rpc('task');
      return !task.running && Boolean(task.error);
    })
    .toBe(true);
  await expect(page.getByRole('alert')).toBeVisible();
  assert.equal((await rpc('snapshot')).instances.length, 1);
  assert.deepEqual(failures, []);
  await writeFile(
    join(output, 'proof.json'),
    JSON.stringify(
      {
        fixture,
        bridge: 'built Vue frontend -> stdio -> same Rust Service used by Tauri',
        desktopControl: false,
        javaOrGameStarted: false,
        handwrittenFixtures: true,
        passed: [
          'snapshot',
          'invalid settings do not change stored configuration',
          'traversal rejection',
          'split launch selector',
          '400ms animated inert input gate',
          'account persisted',
          'settings persisted',
          'theme persisted',
          'instance memory, fullscreen and JVM overrides persist without changing global settings',
          'theme changes preserve unsaved instance settings',
          'restoring inheritance only persists after explicit save',
          'mod rename round trip',
          'compatibility cancel and confirmation',
          'modal focus and Escape',
          '800px layout',
          'reduced motion persists and removes transition duration',
          'failed background task clears busy state and displays error',
        ],
        animation,
      },
      null,
      2,
    ),
  );
  console.log(`Web UI integration passed. Proof: ${join(output, 'proof.json')}`);
} finally {
  if (browser) await browser.close();
  server.closeAllConnections();
  await new Promise((resolve) => server.close(resolve));
  backend.stdin.end();
  if (backend.exitCode === null) {
    const timeout = setTimeout(() => backend.kill(), 2000);
    await once(backend, 'exit');
    clearTimeout(timeout);
  }
}
