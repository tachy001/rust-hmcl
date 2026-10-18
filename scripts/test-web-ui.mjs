// Real built frontend -> JSON-lines harness -> shared Rust Service.
// Uses a headless browser and independent handwritten fixtures, never the desktop.
// --online-modrinth additionally downloads a mod into that isolated fixture; it never executes it.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createInterface } from 'node:readline';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, writeFile, stat } from 'node:fs/promises';
import { resolve, join, sep, extname } from 'node:path';
import { emphasized } from '../frontend/src/motion.ts';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
const onlineModrinth = process.argv.includes('--online-modrinth');
let modrinthProof = null;

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
await mkdir(join(game, 'saves', 'UI World'), { recursive: true });
await writeFile(join(game, 'saves', 'UI World', 'level.dat'), 'handwritten world fixture');
await writeFile(join(version, '1.20.1.jar'), 'handwritten client fixture; never launched');
await writeFile(
  join(fixture, 'config.json'),
  JSON.stringify({
    game_dir: join(fixture, 'minecraft'),
    selected: '1.20.1',
    player: 'PreviewUser',
    isolated: true,
    dark_mode: false,
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
    const allowedModInstall =
      onlineModrinth && args?.job?.kind === 'install_mod' && args?.job?.id === 'online-fabric';
    if (
      command === 'start_job' &&
      !['duplicate', 'rename'].includes(args?.job?.kind) &&
      !allowedModInstall
    )
      throw Error('Headless UI verification never starts Java, games, downloads or installers.');
    return rpc(command, args ?? {});
  });
  await context.addInitScript(() => {
    const callbacks = new Map();
    const listeners = new Map();
    let callbackId = 0;
    window.__hmclWindowState = { maximized: false, fullscreen: false };
    window.__hmclResize = () => {
      for (const handler of listeners.values())
        callbacks.get(handler)?.({
          event: 'tauri://resize',
          payload: { width: innerWidth, height: innerHeight },
        });
    };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: (_event, id) => listeners.delete(id),
    };
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback) => {
        callbacks.set(++callbackId, callback);
        return callbackId;
      },
      unregisterCallback: (id) => callbacks.delete(id),
      invoke: async (command, args) => {
        if (command === 'plugin:window|is_maximized') return window.__hmclWindowState.maximized;
        if (command === 'plugin:window|is_fullscreen') return window.__hmclWindowState.fullscreen;
        if (command === 'plugin:event|listen') {
          listeners.set(args.handler, args.handler);
          return args.handler;
        }
        if (command === 'plugin:event|unlisten') return;
        return window.__hmclInvoke(command, JSON.parse(JSON.stringify(args ?? {})));
      },
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
      Promise.all(
        document
          .getAnimations()
          .filter((animation) => animation.effect?.getTiming().iterations !== Infinity)
          .map((animation) => animation.finished.catch(() => {})),
      ),
    );
  await settle();
  await page.screenshot({ path: join(output, 'home-light.png') });
  assert.equal(
    await page
      .locator('.floating-island')
      .evaluate((element) => getComputedStyle(element).animationName),
    'island-drift',
  );
  await page.getByRole('button', { name: '收藏 1.20.1', exact: true }).click();
  await expect(page.getByRole('button', { name: '取消收藏 1.20.1', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  assert.ok(
    JSON.parse(await readFile(join(fixture, 'config.json'), 'utf8')).favorites.includes('1.20.1'),
  );
  await expect(page.locator('.launcher')).toHaveCSS('border-radius', '12px');
  await page.evaluate(() => {
    window.__hmclWindowState.maximized = true;
    window.__hmclResize();
  });
  await expect(page.locator('.launcher')).toHaveCSS('border-radius', '0px');
  await page.evaluate(() => {
    window.__hmclWindowState.maximized = false;
    window.__hmclWindowState.fullscreen = true;
    window.__hmclResize();
  });
  await expect(page.locator('.launcher')).toHaveCSS('border-radius', '0px');
  await page.evaluate(() => {
    window.__hmclWindowState.fullscreen = false;
    window.__hmclResize();
  });
  await expect(page.locator('.launcher')).toHaveCSS('border-radius', '12px');
  await page.getByRole('button', { name: '选择游戏版本', exact: true }).click();
  await expect(page.getByRole('menu')).toBeVisible();
  await expect(page.locator('.version-popup button[aria-current="true"]')).toBeFocused();
  await page.keyboard.press('End');
  await expect(page.getByRole('menuitem', { name: '安装新版本', exact: true })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button', { name: '选择游戏版本', exact: true })).toBeFocused();
  await page.getByRole('button', { name: '选择游戏版本', exact: true }).click();
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
  const isolationSwitch = page.getByRole('checkbox', { name: '版本隔离', exact: true });
  assert.equal(
    await isolationSwitch.evaluate(
      (element) => getComputedStyle(element, '::before').transitionDuration,
    ),
    '0.2s, 0.2s',
  );
  await isolationSwitch.uncheck();
  await expect(isolationSwitch).not.toBeChecked();
  await settle();
  await isolationSwitch.check();
  const switchMotion = await isolationSwitch.evaluate((element) => {
    const animation = element
      .getAnimations({ subtree: true })
      .find((item) => item.transitionProperty === 'transform');
    if (!animation) throw new Error('Switch thumb has no active transform transition');
    animation.pause();
    const frames = [0, 100, 200].map((time) => {
      animation.currentTime = time;
      return new DOMMatrix(getComputedStyle(element, '::before').transform).m41;
    });
    animation.finish();
    return frames;
  });
  assert.equal(switchMotion[0], 0);
  assert.ok(switchMotion[1] > 0 && switchMotion[1] < 20);
  assert.equal(switchMotion[2], 20);
  await page.getByLabel('最大内存（MB）', { exact: true }).fill('3072');
  await expect(page.getByLabel('最大内存（MB）', { exact: true })).toHaveValue('3072');
  await page.getByRole('button', { name: '保存设置', exact: true }).click();
  await expect
    .poll(async () => JSON.parse(await readFile(join(fixture, 'config.json'), 'utf8')).memory)
    .toBe(3072);
  await expect(page.getByText('设置已保存', { exact: true })).toBeVisible();
  await settle();
  await page.screenshot({ path: join(output, 'settings-light.png') });
  await page.getByRole('button', { name: '切换主题', exact: true }).click();
  await expect(page.locator('.launcher')).toHaveClass(/dark/);
  await page.locator('.page').evaluate((element) => (element.scrollTop = 0));
  await settle();
  await page.screenshot({ path: join(output, 'settings-dark.png') });
  await page.getByRole('button', { name: '首页', exact: true }).click();
  await expect(page.locator('.page').last()).not.toHaveAttribute('inert');
  await settle();
  await page.screenshot({ path: join(output, 'home-dark.png') });
  await page.setViewportSize({ width: 800, height: 560 });
  await settle();
  assert.equal(
    await page
      .locator('.page')
      .last()
      .evaluate((element) => element.scrollWidth > element.clientWidth),
    false,
    'Aster home must fit the narrow viewport without horizontal scrolling',
  );
  await page.screenshot({ path: join(output, 'home-dark-narrow.png') });
  await page.setViewportSize({ width: 1080, height: 700 });
  await page.getByRole('button', { name: '游戏库', exact: true }).click();
  await page.getByRole('button', { name: '只看收藏', exact: true }).click();
  await expect(page.locator('.library-list .list-row')).toHaveCount(1);
  await page.getByRole('button', { name: '取消收藏 1.20.1', exact: true }).click();
  await expect(page.getByText('这里暂时没有匹配的游戏', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '显示全部游戏', exact: true }).click();
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
  const modFilter = page.getByRole('searchbox', { name: '筛选已安装模组', exact: true });
  await modFilter.fill('does-not-exist');
  await expect(page.getByText('没有匹配的模组', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '清空筛选', exact: true }).click();
  await expect(page.getByText('sample.jar', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '发现模组', exact: true }).click();
  await expect(page.getByText('此实例暂不能在线安装模组', { exact: true })).toBeVisible();
  assert.equal(await page.getByRole('button', { name: '搜索模组', exact: true }).count(), 0);
  await settle();
  await page.screenshot({ path: join(output, 'mod-discover-vanilla.png') });
  await page.getByRole('button', { name: '已安装模组', exact: false }).click();
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
  assert.equal(
    await isolationSwitch.evaluate(
      (element) => getComputedStyle(element, '::before').transitionDuration,
    ),
    '0s',
  );
  await page.getByRole('button', { name: '游戏库', exact: true }).click();
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
  await page.getByRole('button', { name: '关闭提示', exact: true }).click();
  let beforeCopy = await rpc('snapshot');
  await rpc('update_config', {
    config: {
      ...beforeCopy.config,
      favorites: ['1.20.1'],
      version_settings: { ...beforeCopy.config.version_settings, '1.20.1': { memory: 4096 } },
    },
  });
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  const more = page.getByRole('button', { name: '1.20.1更多操作', exact: true });
  await more.click();
  await settle();
  await page.screenshot({ path: join(output, 'instance-menu-dark-narrow.png') });
  await expect(page.getByRole('menuitem', { name: '测试游戏', exact: true })).toBeFocused();
  await page.keyboard.press('End');
  await expect(page.getByRole('menuitem', { name: '复制实例', exact: true })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(more).toBeFocused();
  await expect(page.getByRole('menu')).toHaveCount(0);
  await more.click({ button: 'right' });
  await page.getByRole('menuitem', { name: '复制实例', exact: true }).click();
  const nameInput = page.getByRole('textbox', { name: '新实例名称', exact: true });
  await expect(nameInput).toBeFocused();
  await expect(page.getByRole('checkbox', { name: '复制存档', exact: true })).not.toBeChecked();
  await page.getByRole('button', { name: '确认', exact: true }).click();
  await expect(page.getByText('新名称与原名称相同。', { exact: true })).toBeVisible();
  await nameInput.fill('../bad');
  await page.getByRole('button', { name: '确认', exact: true }).click();
  await expect(nameInput).toHaveAttribute('aria-invalid', 'true');
  await nameInput.fill('copied-no-worlds');
  await settle();
  await page.screenshot({ path: join(output, 'copy-dialog-dark-narrow.png') });
  await page.getByRole('button', { name: '确认', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByText('copied-no-worlds', { exact: true })).toBeVisible();
  const noWorlds = join(fixture, 'minecraft', 'versions', 'copied-no-worlds');
  await assert.rejects(stat(join(noWorlds, 'game', 'saves')));
  assert.deepEqual(
    await readFile(join(noWorlds, 'game', 'mods', 'sample.jar')),
    await readFile(join(game, 'mods', 'sample.jar')),
  );
  let afterCopy = await rpc('snapshot');
  assert.equal(afterCopy.config.selected, 'copied-no-worlds');
  assert.equal(afterCopy.config.version_settings['copied-no-worlds'].memory, 4096);
  assert.equal(afterCopy.config.version_settings['copied-no-worlds'].isolated, true);
  assert.ok(afterCopy.config.favorites.includes('copied-no-worlds'));
  await more.click();
  await page.getByRole('menuitem', { name: '复制实例', exact: true }).click();
  await nameInput.fill('copied-with-worlds');
  await page.getByRole('checkbox', { name: '复制存档', exact: true }).check();
  await page.getByRole('button', { name: '确认', exact: true }).click();
  await expect(page.getByText('copied-with-worlds', { exact: true })).toBeVisible();
  assert.deepEqual(
    await readFile(
      join(
        fixture,
        'minecraft',
        'versions',
        'copied-with-worlds',
        'game',
        'saves',
        'UI World',
        'level.dat',
      ),
    ),
    await readFile(join(game, 'saves', 'UI World', 'level.dat')),
  );
  const dependent = join(fixture, 'minecraft', 'versions', 'dependent');
  await mkdir(dependent);
  await writeFile(
    join(dependent, 'dependent.json'),
    JSON.stringify({ id: 'dependent', inheritsFrom: 'copied-no-worlds', libraries: [] }),
  );
  await page.getByRole('button', { name: 'copied-no-worlds更多操作', exact: true }).click();
  await page.getByRole('menuitem', { name: '重命名实例', exact: true }).click();
  await nameInput.fill('renamed-ui');
  await page.getByRole('button', { name: '确认', exact: true }).click();
  await expect(page.getByText('renamed-ui', { exact: true })).toBeVisible();
  await assert.rejects(stat(noWorlds));
  afterCopy = await rpc('snapshot');
  assert.equal(afterCopy.config.selected, 'renamed-ui');
  assert.equal(afterCopy.config.version_settings['renamed-ui'].memory, 4096);
  assert.equal(afterCopy.config.version_settings['copied-no-worlds'], undefined);
  assert.ok(afterCopy.config.favorites.includes('renamed-ui'));
  assert.ok(!afterCopy.config.favorites.includes('copied-no-worlds'));
  assert.equal(
    JSON.parse(await readFile(join(dependent, 'dependent.json'), 'utf8')).inheritsFrom,
    'renamed-ui',
  );
  assert.equal(afterCopy.config.version_settings.untouched.memory, 8192);
  assert.equal(
    await readFile(join(version, '1.20.1.jar'), 'utf8'),
    'handwritten client fixture; never launched',
  );
  await page.getByRole('button', { name: 'renamed-ui更多操作', exact: true }).click();
  await page.getByRole('menuitem', { name: '重命名实例', exact: true }).click();
  await nameInput.fill('1.20.1');
  await page.getByRole('button', { name: '确认', exact: true }).click();
  await expect(page.getByText('同名实例已存在，不能覆盖。', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '取消', exact: true }).click();
  await page.setViewportSize({ width: 1080, height: 700 });
  await settle();
  await page.screenshot({ path: join(output, 'instances-dark.png') });
  if (onlineModrinth) {
    const fabric = join(fixture, 'minecraft', 'versions', 'online-fabric');
    await mkdir(fabric, { recursive: true });
    await writeFile(
      join(fabric, 'online-fabric.json'),
      JSON.stringify({
        id: 'online-fabric',
        inheritsFrom: '1.20.1',
        libraries: [{ name: 'net.fabricmc:fabric-loader:0.16.0' }],
      }),
    );
    const config = (await rpc('snapshot')).config;
    await rpc('update_config', {
      config: {
        ...config,
        version_settings: { ...config.version_settings, 'online-fabric': { isolated: true } },
      },
    });
    await page.getByRole('button', { name: '刷新', exact: true }).click();
    await page.getByRole('button', { name: '管理 online-fabric', exact: true }).click();
    await page.getByRole('button', { name: '模组管理', exact: true }).click();
    await page.getByRole('button', { name: '发现模组', exact: true }).first().click();
    await expect(page.getByText('1.20.1 · fabric', { exact: true })).toBeVisible();
    await page
      .getByRole('searchbox', { name: '搜索 Modrinth 模组', exact: true })
      .fill('fabric-api');
    await page.getByRole('button', { name: '搜索模组', exact: true }).click();
    const install = page.getByRole('button', { name: '安装 Fabric API', exact: true });
    await expect(install).toBeVisible({ timeout: 60000 });
    await settle();
    await page.screenshot({ path: join(output, 'modrinth-results-dark.png') });
    await page.setViewportSize({ width: 800, height: 560 });
    await settle();
    assert.equal(
      await page
        .locator('.page')
        .last()
        .evaluate((element) => element.scrollWidth > element.clientWidth),
      false,
    );
    await page.screenshot({ path: join(output, 'modrinth-results-narrow.png') });
    const beforeInstall = (await rpc('task')).id;
    await install.click();
    await expect
      .poll(async () => (await rpc('task')).id, { timeout: 10000 })
      .toBeGreaterThan(beforeInstall);
    await expect.poll(async () => (await rpc('task')).running, { timeout: 120000 }).toBe(false);
    const finished = await rpc('task');
    assert.equal(finished.error, null);
    const installed = await rpc('mod_files');
    assert.ok(
      installed.some((mod) => mod.name.startsWith('fabric-api') && mod.enabled && mod.size > 10000),
    );
    const registry = JSON.parse(
      await readFile(join(fabric, 'game', 'mods', '.rust-modrinth.json'), 'utf8'),
    );
    modrinthProof = {
      transport: 'real official Modrinth API and CDN',
      instance: 'online-fabric',
      game: '1.20.1',
      loader: 'fabric',
      files: [],
    };
    for (const entry of Object.values(registry)) {
      const bytes = await readFile(join(fabric, 'game', 'mods', entry.filename));
      modrinthProof.files.push({
        filename: entry.filename,
        version: entry.version,
        bytes: bytes.length,
        sha1: createHash('sha1').update(bytes).digest('hex'),
      });
    }
    await page.getByRole('button', { name: '已安装模组', exact: false }).click();
    await page.getByRole('button', { name: '刷新', exact: true }).click();
    await expect(page.getByText(installed[0].name, { exact: true })).toBeVisible();
    await page.setViewportSize({ width: 1080, height: 700 });
    await settle();
    await page.screenshot({ path: join(output, 'modrinth-installed.png') });
  }
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
        modrinthProof,
        passed: [
          'snapshot',
          'Aster home favorites persist and library filter updates after unpinning',
          'original island scene uses transform animation',
          'Aster version picker keyboard navigation and focus restoration',
          'window corners become square when maximized or fullscreen and restore on resize',
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
          'local mod search and vanilla discovery guard without network',
          'compatibility cancel and confirmation',
          'modal focus and Escape',
          '800px layout',
          'reduced motion persists and removes transition duration',
          'failed background task clears busy state and displays error',
          'instance context menu keyboard navigation and focus restoration',
          'duplicate defaults to no saved worlds; opt-in copies exact world bytes',
          'duplicate and rename preserve settings, favorites, original files and dependent inheritance',
          'invalid and conflicting instance names leave dialog open and never overwrite',
          'custom switch interaction and pseudo-element motion honor reduced animation settings',
        ],
        animation,
        switchMotion,
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
