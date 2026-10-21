// Opt-in official metadata checks. Handwritten instance metadata; no Java or game downloads.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { once } from 'node:events';
import { mkdtemp, mkdir, writeFile, readFile, readdir, copyFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';

class BackendSession {
  pending = [];
  constructor(executable, fixture) {
    this.child = spawn(executable, [], {
      windowsHide: true,
      env: { ...process.env, RUST_HMCL_DATA_DIR: fixture },
      stdio: ['pipe', 'pipe', 'inherit'],
    });
    this.child.on('error', (error) => this.fail(error));
    this.child.on('exit', (code) => this.fail(new Error(`Service exited: ${code}`)));
    createInterface({ input: this.child.stdout }).on('line', (line) => {
      const request = this.pending.shift();
      if (!request) return;
      try {
        const response = JSON.parse(line);
        response.ok ? request.resolve(response.value) : request.reject(new Error(response.error));
      } catch (error) {
        request.reject(error);
      }
    });
  }
  fail(error) {
    while (this.pending.length) this.pending.shift().reject(error);
  }
  request(method, args = {}) {
    assert.ok(['loader_versions', 'snapshot', 'task', 'start_job'].includes(method));
    if (method === 'start_job') {
      assert.ok(['fabric', 'quilt'].includes(args.job.kind));
      assert.equal(args.job.version, '../bad'); // Guaranteed preflight rejection, never an installation.
    }
    return new Promise((resolve, reject) => {
      this.pending.push({ resolve, reject });
      this.child.stdin.write(JSON.stringify({ method, ...args }) + '\n');
    });
  }
  async close() {
    this.child.stdin.end();
    if (this.child.exitCode === null) {
      const timeout = setTimeout(() => this.child.kill(), 3000);
      await once(this.child, 'exit');
      clearTimeout(timeout);
    }
  }
}

const root = resolve(import.meta.dirname, '..');
await mkdir(join(root, '.tools'), { recursive: true });
const fixture = await mkdtemp(join(root, '.tools', 'loader-catalog-proof-'));
const game = join(fixture, 'minecraft');
const id = 'Vanilla Alias';
const version = join(game, 'versions', id);
await mkdir(version, { recursive: true });
const profilePath = join(version, `${id}.json`);
await writeFile(
  profilePath,
  JSON.stringify({
    id,
    rustHmclGameVersion: '1.20.1',
    javaVersion: { majorVersion: 17 },
    libraries: [],
  }),
);
const configPath = join(fixture, 'config.json');
await writeFile(configPath, JSON.stringify({ game_dir: game, selected: id, auto_java: false }));
const configBefore = await readFile(configPath);
const profileBefore = await readFile(profilePath);
const executable = join(fixture, process.platform === 'win32' ? 'service.exe' : 'service');
await copyFile(
  process.env.HMCL_SERVICE_BIN ??
    join(
      root,
      '..',
      'target',
      'debug',
      process.platform === 'win32' ? 'hmcl-service.exe' : 'hmcl-service',
    ),
  executable,
);
const session = new BackendSession(executable, fixture);
const reports = [];
try {
  const taskBefore = await session.request('task');
  for (const loader of ['fabric', 'quilt', 'forge']) {
    const result = await session.request('loader_versions', { id, loader });
    assert.equal(result.instance_id, id);
    assert.equal(result.minecraft, '1.20.1');
    assert.equal(result.loader, loader);
    assert.equal(result.install_supported, true);
    assert.equal(result.reason, null);
    assert.ok(result.versions.length > 1);
    assert.equal(new Set(result.versions.map((r) => r.version)).size, result.versions.length);
    const recommended = result.versions.filter((r) => r.recommended);
    const latest = result.versions.filter((r) => r.latest);
    assert.equal(recommended.length, 1);
    assert.equal(latest.length, 1);
    if (loader !== 'forge') {
      const url =
        loader === 'fabric'
          ? 'https://meta.fabricmc.net/v2/versions/loader/1.20.1'
          : 'https://meta.quiltmc.org/v3/versions/loader/1.20.1';
      const response = await fetch(url, { signal: AbortSignal.timeout(30000) });
      assert.equal(response.status, 200);
      const official = (await response.json()).map((r) => r.loader);
      const stable = official.filter((r) => r.stable ?? !r.version.includes('-'));
      stable.sort((a, b) => b.version.localeCompare(a.version, 'en', { numeric: true }));
      assert.equal(recommended[0].version, stable[0].version);
      assert.equal(recommended[0].stable, true);
    } else {
      const response = await fetch(
        'https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json',
        { signal: AbortSignal.timeout(30000) },
      );
      assert.equal(response.status, 200);
      const { promos } = await response.json();
      assert.equal(recommended[0].version, promos['1.20.1-recommended'] ?? promos['1.20.1-latest']);
    }
    reports.push({
      loader,
      count: result.versions.length,
      recommended: recommended[0].version,
      latest: latest[0].version,
    });
  }
  for (const invalid of ['../escape', 'missing']) {
    await assert.rejects(session.request('loader_versions', { id: invalid, loader: 'fabric' }));
  }
  for (const kind of ['fabric', 'quilt']) {
    await assert.rejects(session.request('start_job', { job: { kind, id, version: '../bad' } }));
  }
  assert.deepEqual(await session.request('task'), taskBefore);
  assert.equal((await session.request('snapshot')).config.selected, id);
  assert.deepEqual(await readFile(configPath), configBefore);
  assert.deepEqual(await readFile(profilePath), profileBefore);
  assert.deepEqual(await readdir(game), ['versions']);
  assert.deepEqual(await readdir(version), [`${id}.json`]);
  await mkdir(join(root, 'dist'), { recursive: true });
  await writeFile(
    join(root, 'dist', 'loader-catalog-online-proof.json'),
    JSON.stringify(
      {
        fixture,
        handwrittenInstanceMetadata: true,
        officialMetadata: true,
        reports,
        serviceSha256: createHash('sha256')
          .update(await readFile(executable))
          .digest('hex'),
        configurationAndInstancePreserved: true,
        taskUnchanged: true,
        gameDownloads: false,
        javaOrGameStarted: false,
        desktopInput: false,
        verifiedAt: new Date().toISOString(),
      },
      null,
      2,
    ),
  );
  console.log('Official loader catalog checks passed:', JSON.stringify(reports));
} finally {
  await session.close();
}
