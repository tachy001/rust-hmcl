// Component fixtures verify presentation/interaction only, not a real Java/game run.
// Service integration is covered separately by test-web-ui.mjs and test-native-ui.mjs.
import { createServer } from 'vite';
import { chromium, expect } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import assert from 'node:assert/strict';
const root = resolve(import.meta.dirname, '..');
const output = join(root, 'dist', 'console-component-proof');
await mkdir(output, { recursive: true });
const task = {
  id: 7,
  launch_id: 'FixtureWorld',
  running: false,
  game_running: false,
  progress: 0,
  status: '任务失败',
  error: '游戏异常退出 (exit code: 1)',
  clues: [
    {
      title: 'Java 堆内存耗尽',
      evidence: 'java.lang.OutOfMemoryError: Java heap space',
      suggestion: '检查实例内存与模组需求；调整前确认系统还有足够可用内存。',
      action: 'settings',
    },
  ],
  logs: [
    ...Array.from({ length: 90 }, (_, i) => `[main/INFO] Output line ${i}`),
    '[main/WARN] Fixture warning',
    '[main/ERROR] java.lang.OutOfMemoryError: Java heap space',
    '<img src=x onerror="window.__unsafe=true">',
  ],
};
const html = `<!doctype html><html><head><meta charset="utf-8"></head><body>
<div class="launcher" style="display:block;height:100vh;overflow:auto;padding:24px"><div id="fixture"></div></div>
<script type="module">
import {createApp,reactive,h} from 'vue';
import Console from '/frontend/src/LaunchConsole.vue';
import '/frontend/src/style.css';import '/frontend/src/aster.css';
const task=reactive(${JSON.stringify(task).replaceAll('<', '\\u003c')});
window.__consoleFixture={task,recovered:[]};
window.__TAURI_INTERNALS__={invoke:async()=>null};
createApp({render:()=>h(Console,{task,recoverable:true,onRecover:(action)=>window.__consoleFixture.recovered.push(action)})}).mount('#fixture');
</script></body></html>`;
const server = await createServer({
  root,
  publicDir: join(root, 'assets'),
  server: { host: '127.0.0.1', port: 0 },
  plugins: [
    {
      name: 'console-fixture',
      configureServer(server) {
        server.middlewares.use(async (req, res, next) => {
          if (req.url?.split('?')[0] !== '/__console-fixture.html') return next();
          try {
            res.setHeader('Content-Type', 'text/html; charset=utf-8');
            res.end(await server.transformIndexHtml('/__console-fixture.html', html));
          } catch (error) {
            next(error);
          }
        });
      },
    },
  ],
});
let browser;
try {
  await server.listen();
  const address = server.httpServer.address();
  browser = await chromium.launch(
    process.platform === 'win32' ? { channel: 'msedge', headless: true } : { headless: true },
  );
  const page = await browser.newPage({ viewport: { width: 1080, height: 900 } });
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${address.port}/__console-fixture.html`);
  await expect(page.locator('.diagnosis-clue')).toContainText('Java 堆内存耗尽');
  await expect(page.locator('.console-counts')).toContainText('错误 1 · 警告 1');
  await expect(page.locator('.console-line')).toHaveCount(93);
  await expect(page.locator('.console-output img')).toHaveCount(0);
  assert.equal(await page.evaluate(() => window.__unsafe), undefined);
  await page.getByRole('button', { name: '检查实例设置', exact: true }).click();
  assert.deepEqual(await page.evaluate(() => window.__consoleFixture.recovered), ['settings']);
  await page.getByRole('combobox', { name: '日志级别', exact: true }).selectOption('error');
  await expect(page.locator('.console-line')).toHaveCount(1);
  await expect(page.locator('.console-line')).toContainText('OutOfMemoryError');
  await page.getByRole('searchbox', { name: '搜索日志', exact: true }).fill('no-such-line');
  await expect(page.locator('.console-empty')).toContainText('没有匹配的输出');
  await page.getByRole('searchbox', { name: '搜索日志', exact: true }).fill('');
  await page.getByRole('combobox', { name: '日志级别', exact: true }).selectOption('all');
  await page.locator('.console-output').evaluate((el) => {
    el.scrollTop = 0;
    el.dispatchEvent(new Event('scroll'));
  });
  await expect(page.getByRole('button', { name: '继续跟随', exact: true })).toHaveAttribute(
    'aria-pressed',
    'false',
  );
  await page.evaluate(() =>
    window.__consoleFixture.task.logs.push('[main/INFO] arrived while paused'),
  );
  assert.equal(await page.locator('.console-output').evaluate((el) => el.scrollTop), 0);
  await page.getByRole('button', { name: '继续跟随', exact: true }).click();
  await expect
    .poll(() =>
      page
        .locator('.console-output')
        .evaluate((el) => Math.abs(el.scrollHeight - el.scrollTop - el.clientHeight)),
    )
    .toBeLessThan(2);
  await page.screenshot({ path: join(output, 'launch-diagnosis-light.png'), fullPage: true });
  await page.evaluate(() => document.querySelector('.launcher').classList.add('dark'));
  await page.screenshot({ path: join(output, 'launch-diagnosis-dark.png'), fullPage: true });
  await page.setViewportSize({ width: 800, height: 560 });
  assert.equal(
    await page.locator('.launch-console').evaluate((el) => el.scrollWidth <= el.clientWidth),
    true,
  );
  await page.screenshot({ path: join(output, 'launch-diagnosis-narrow.png'), fullPage: true });
  assert.deepEqual(errors, []);
  await writeFile(
    join(output, 'proof.json'),
    JSON.stringify(
      {
        syntheticComponentFixture: true,
        realGameOrJavaStarted: false,
        desktopInput: false,
        passed: [
          'clue evidence rendering and recovery event',
          'error/warning counts',
          'severity and text filtering',
          'untrusted log text stays plain text',
          'manual scroll pauses following',
          'new lines preserve paused position',
          'resume scrolls to newest output',
          'light/dark and narrow layout',
        ],
      },
      null,
      2,
    ),
  );
  console.log(`Console component checks passed: ${output}`);
} finally {
  if (browser) await browser.close();
  await server.close();
}
