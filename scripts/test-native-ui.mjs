// Real release Tauri EXE + WebView2 + native IPC. No desktop input or visible window.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createServer } from 'node:net';
import { mkdtemp, mkdir, readFile, writeFile, stat } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
import { localImportFixtures } from './local-import-fixtures.mjs';
import { modpackFixture } from './modpack-fixtures.mjs';

if (process.platform !== 'win32')
  throw new Error('This native verification uses Windows WebView2.');
const root = resolve(import.meta.dirname, '..');
const executable =
  process.env.HMCL_DESKTOP_BIN ?? join(root, '..', 'target', 'release', 'rust-hmcl-desktop.exe');
await stat(executable);
const tools = join(root, '.tools');
await mkdir(tools, { recursive: true });
const fixture = await mkdtemp(join(tools, 'native-ui-proof-'));
const imports = await localImportFixtures(fixture);
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
    dark_mode: false,
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
  const targetStart = Date.now();
  let pages = browser.contexts().flatMap((context) => context.pages());
  // The browser debugger becomes ready before WebView2 creates its first page on CI.
  // Keep observing this same live app/connection instead of restarting the process.
  while (!pages.length && Date.now() - targetStart < 45000) {
    if (spawnError) throw spawnError;
    if (app.exitCode !== null)
      throw new Error(
        `Desktop exited while waiting for WebView (${app.exitCode}): ${processError}`,
      );
    if (!browser.isConnected())
      throw new Error('Native debugger disconnected before page creation');
    await new Promise((resolve) => setTimeout(resolve, 200));
    pages = browser.contexts().flatMap((context) => context.pages());
  }
  const startupWaitMs = Date.now() - targetStart;
  assert.ok(pages.length, `Native WebView target did not appear in 45 seconds: ${processError}`);
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
  const typography = await page.evaluate(() => ({
    bodyFamily: getComputedStyle(document.body).fontFamily,
    sidebarSize: getComputedStyle(document.querySelector('.sidebar > button.selected')).fontSize,
  }));
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('DOM.enable');
  await cdp.send('CSS.enable');
  const { root: documentRoot } = await cdp.send('DOM.getDocument');
  const { nodeId } = await cdp.send('DOM.querySelector', {
    nodeId: documentRoot.nodeId,
    selector: '.sidebar > button.selected > span:last-child',
  });
  const { fonts } = await cdp.send('CSS.getPlatformFontsForNode', { nodeId });
  typography.renderedFonts = fonts.map(({ familyName, glyphCount }) => ({
    familyName,
    glyphCount,
  }));
  await cdp.detach();
  assert.equal(typography.sidebarSize, '13px');
  assert.ok(
    typography.renderedFonts.some((font) => font.glyphCount >= 2),
    'Chinese navigation glyphs were not rendered',
  );
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
      Promise.all(
        document
          .getAnimations()
          .filter((animation) => animation.effect?.getTiming().iterations !== Infinity)
          .map((animation) => animation.finished.catch(() => {})),
      ),
    );
  await settle();
  await page.screenshot({ path: join(output, 'native-home-light.png'), omitBackground: true });
  await page.getByRole('button', { name: '收藏 native-fixture', exact: true }).click();
  await expect(
    page.getByRole('button', { name: '取消收藏 native-fixture', exact: true }),
  ).toHaveAttribute('aria-pressed', 'true');
  assert.ok((await invoke('snapshot')).config.favorites.includes('native-fixture'));
  const cornerProof = [];
  for (const [name, x, y] of [
    ['top-left', 0, 0],
    ['top-right', layout.width - 24, 0],
    ['bottom-left', 0, layout.height - 24],
    ['bottom-right', layout.width - 24, layout.height - 24],
  ]) {
    const png = await page.screenshot({
      clip: { x, y, width: 24, height: 24 },
      omitBackground: true,
    });
    const samples = await page.evaluate(
      async ({ bytes, name }) => {
        const bitmap = await createImageBitmap(
          new Blob([new Uint8Array(bytes)], { type: 'image/png' }),
        );
        const canvas = document.createElement('canvas');
        canvas.width = bitmap.width;
        canvas.height = bitmap.height;
        const ctx = canvas.getContext('2d');
        ctx.drawImage(bitmap, 0, 0);
        const image = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
        const x = name.endsWith('right') ? canvas.width - 1 : 0;
        const y = name.startsWith('bottom') ? canvas.height - 1 : 0;
        const alpha = image[(y * canvas.width + x) * 4 + 3];
        const antialiased = image.some(
          (value, index) => index % 4 === 3 && value > 0 && value < 255,
        );
        bitmap.close();
        return { name, alpha, antialiased };
      },
      { bytes: [...png], name },
    );
    assert.equal(samples.alpha, 0, `${name} must be transparent`);
    assert.equal(samples.antialiased, true, `${name} must have a smooth edge`);
    cornerProof.push(samples);
  }
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
  await page.screenshot({ path: join(output, 'native-settings-dark.png'), omitBackground: true });
  await page.getByRole('button', { name: '首页', exact: true }).click();
  await expect(page.locator('.page').last()).not.toHaveAttribute('inert');
  await settle();
  await page.screenshot({ path: join(output, 'native-home-dark.png'), omitBackground: true });
  await page.getByRole('button', { name: '游戏库', exact: true }).click();
  await page.getByRole('button', { name: '管理 native-fixture', exact: true }).click();
  await page.getByRole('button', { name: '模组管理', exact: true }).click();
  await page.getByRole('button', { name: '禁用', exact: true }).click();
  await expect(page.getByText('native-sample.jar.disabled', { exact: true })).toBeVisible();
  assert.ok((await stat(join(game, 'mods', 'native-sample.jar.disabled'))).isFile());
  await page.getByRole('button', { name: '启用', exact: true }).click();
  await expect(page.getByText('native-sample.jar', { exact: true })).toBeVisible();
  await page.getByRole('searchbox', { name: '筛选已安装模组', exact: true }).fill('missing-mod');
  await expect(page.getByText('没有匹配的模组', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '清空筛选', exact: true }).click();
  await page.getByRole('button', { name: '发现模组', exact: true }).click();
  await expect(page.getByText('此实例暂不能在线安装模组', { exact: true })).toBeVisible();
  await expect(page.locator('.mod-notice')).toContainText('请先选择 Fabric 或 Quilt');
  const fabricEnvironment = join(fixture, 'minecraft', 'versions', 'native-fabric');
  await mkdir(fabricEnvironment, { recursive: true });
  await writeFile(
    join(fabricEnvironment, 'native-fabric.json'),
    JSON.stringify({
      id: 'native-fabric',
      inheritsFrom: 'native-fixture',
      libraries: [{ name: 'net.fabricmc:fabric-loader:0.16.0' }],
    }),
  );
  assert.deepEqual(await invoke('mod_environment', { id: 'native-fabric' }), {
    game: 'native-fixture',
    loader: 'fabric',
  });
  assert.equal((await invoke('snapshot')).config.selected, 'native-fixture');
  const invalidSearch = await page.evaluate(async () => {
    try {
      await window.__TAURI_INTERNALS__.invoke('search_mods', {
        id: 'native-fixture',
        query: 'sodium',
      });
      return '';
    } catch (error) {
      return String(error);
    }
  });
  assert.match(invalidSearch, /请先选择 Fabric 或 Quilt/);
  await settle();
  await page.screenshot({ path: join(output, 'native-mod-discovery.png'), omitBackground: true });
  const invalidMod = await page.evaluate(async () => {
    try {
      await window.__TAURI_INTERNALS__.invoke('start_job', {
        job: { kind: 'install_mod', id: 'native-fixture', project: '../invalid' },
      });
      return false;
    } catch {
      return true;
    }
  });
  assert.equal(invalidMod, true);
  assert.equal((await invoke('task')).running, false);
  await page.getByRole('button', { name: '已安装模组', exact: false }).click();
  await expect(page.getByRole('region', { name: '导入本地模组', exact: true })).toBeVisible();
  await settle();
  const emitDrop = async (paths) => {
    await invoke('plugin:event|emit_to', {
      target: { kind: 'Webview', label: 'main' },
      event: 'tauri://drag-drop',
      payload: { paths, position: { x: 100, y: 100 } },
    });
  };
  const importPanel = page.getByRole('region', { name: '导入本地模组', exact: true });
  await expect(importPanel).toContainText('选择或拖入');
  await emitDrop([imports.mod]);
  await expect(importPanel.getByRole('listitem')).toHaveCount(1);
  await page.screenshot({ path: join(output, 'native-local-import.png'), omitBackground: true });
  await importPanel.getByRole('button', { name: '导入 1 项', exact: true }).click();
  await expect(
    page.locator('.mod-library .list').getByText('imported-sample.jar', { exact: true }),
  ).toBeVisible();
  assert.deepEqual(
    await readFile(join(game, 'mods', 'imported-sample.jar')),
    await readFile(imports.mod),
  );
  const beforeDuplicate = (await invoke('task')).id;
  await emitDrop([imports.mod]);
  await importPanel.getByRole('button', { name: '导入 1 项', exact: true }).click();
  await expect(page.locator('.error-banner')).toContainText('已存在，不会覆盖');
  assert.equal((await invoke('task')).id, beforeDuplicate);
  await importPanel.getByRole('button', { name: '清空', exact: true }).click();
  const pickerBlocked = await page.evaluate(async () => {
    try {
      await window.__TAURI_INTERNALS__.invoke('pick_import', { content: 'mods', folder: false });
      return '';
    } catch (error) {
      return String(error);
    }
  });
  assert.match(pickerBlocked, /隐藏验证模式不打开文件选择器/);
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
  const originalOptions = await readFile(join(game, 'options.txt'));
  const resourceImport = page.getByRole('region', { name: '导入本地资源包', exact: true });
  await expect(resourceImport).toContainText('选择或拖入');
  await emitDrop([imports.zip, imports.folder]);
  await expect(resourceImport.getByRole('listitem')).toHaveCount(2);
  await resourceImport.getByRole('button', { name: '导入 2 项', exact: true }).click();
  await expect(
    page.getByRole('checkbox', { name: '启用 Imported ZIP', exact: true }),
  ).not.toBeChecked();
  await expect(
    page.getByRole('checkbox', { name: '启用 Imported Folder', exact: true }),
  ).not.toBeChecked();
  assert.deepEqual(
    await readFile(join(game, 'resourcepacks', 'Imported ZIP.zip')),
    await readFile(imports.zip),
  );
  assert.deepEqual(
    await readFile(join(game, 'resourcepacks', 'Imported Folder', 'assets', 'sample.txt')),
    await readFile(join(imports.folder, 'assets', 'sample.txt')),
  );
  assert.deepEqual(await readFile(join(game, 'options.txt')), originalOptions);
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
  const pack = await modpackFixture(fixture);
  await page.getByRole('button', { name: '整合包', exact: true }).click();
  await expect(page.locator('.pack-drop-zone')).toContainText('选择或拖入');
  await settle();
  await emitDrop([pack.path]);
  await expect(page.getByRole('heading', { name: 'Aster Journey', exact: true })).toBeVisible();
  const packName = page.getByRole('textbox', { name: '整合包实例名称', exact: true });
  await packName.fill('Aster Native Pack');
  await page
    .locator('.page')
    .last()
    .evaluate((element) => (element.scrollTop = 0));
  await page.screenshot({ path: join(output, 'native-modpack-preview.png'), omitBackground: true });
  await page.getByRole('button', { name: '安装到新实例', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Aster Native Pack 已准备好', exact: true }),
  ).toBeVisible({ timeout: 10000 });
  const packDir = join(fixture, 'minecraft', 'versions', 'Aster Native Pack');
  await expect(
    page.getByRole('heading', { name: 'Aster Native Pack 已准备好', exact: true }),
  ).toBeInViewport();
  await assert.rejects(stat(join(packDir, 'pack-staging')));
  assert.equal(
    await readFile(join(packDir, 'game', 'config', 'aster.txt'), 'utf8'),
    'client override wins',
  );
  const afterPack = await invoke('snapshot');
  assert.equal(afterPack.config.selected, 'Aster Native Pack');
  assert.equal(afterPack.config.version_settings['Aster Native Pack'].isolated, true);
  assert.equal(
    createHash('sha1')
      .update(await readFile(pack.path))
      .digest('hex'),
    pack.sha1,
  );
  await assert.rejects(stat(join(packDir, 'game', 'mods', 'optional.jar')));
  await assert.rejects(stat(join(packDir, 'game', 'mods', 'server.jar')));
  await page.screenshot({
    path: join(output, 'native-modpack-installed.png'),
    omitBackground: true,
  });
  assert.deepEqual(errors, []);
  const proof = {
    executable,
    executableSha256: createHash('sha256')
      .update(await readFile(executable))
      .digest('hex'),
    fixture,
    transport: 'real release Tauri / WebView2 / native IPC',
    layout,
    typography,
    cornerProof,
    nativeSize,
    url: page.url(),
    nativeWindowVisible: false,
    startupWaitMs,
    desktopInput: false,
    javaOrGameStarted: false,
    handwrittenFixtures: true,
    passed: [
      'hidden native window',
      'Aster home favorite persists through real native IPC',
      'four transparent antialiased corners without opaque page background',
      'bundled Vue assets and CSP',
      'snapshot IPC',
      'invalid configuration does not write',
      'traversal rejected before game start',
      'offline account persistence',
      'global settings persistence',
      'theme',
      'mod toggle round trip',
      'local mod filtering, native loader guard and invalid project rejection',
      'synthetic native drop events drive local import preview, exclusive copy and conflict feedback',
      'ZIP and folder resource imports retain sources and do not enable automatically',
      'mrpack preview and cached native installation with isolated config and client overrides',
      'resource compatibility confirmation before options write',
      'native background copy task and completion refresh',
    ],
    limitations: [
      'Windows only',
      'visible compositor edges, corner hit testing and native maximization not verified',
      'Java/game launching and remote game/loader downloads not exercised; mrpack uses cached handwritten samples',
      'native file picker and physical OS drag gestures not exercised; drop event payloads are synthetic',
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
