// Opt-in real Temurin download and version-only probes in an isolated data directory.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { once } from 'node:events';
import {
  mkdtemp,
  mkdir,
  copyFile,
  readFile,
  writeFile,
  readdir,
  stat,
  realpath,
} from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve, join, sep } from 'node:path';
import assert from 'node:assert/strict';
const root = resolve(import.meta.dirname, '..');
const tools = join(root, '.tools');
await mkdir(tools, { recursive: true });
const reuse = process.argv.indexOf('--fixture');
const fixture =
  reuse >= 0
    ? await realpath(resolve(process.argv[reuse + 1]))
    : await mkdtemp(join(tools, 'java-online-proof-'));
assert.ok(
  fixture.startsWith((await realpath(tools)) + sep) &&
    /java-online-proof-[a-z0-9]+$/i.test(fixture),
);
const major = 17;
const binary =
  process.env.HMCL_SERVICE_BIN ??
  join(
    root,
    '..',
    'target',
    'debug',
    process.platform === 'win32' ? 'hmcl-service.exe' : 'hmcl-service',
  );
const executable = join(fixture, process.platform === 'win32' ? 'service.exe' : 'service');
await copyFile(binary, executable);
if (reuse < 0)
  await writeFile(
    join(fixture, 'config.json'),
    JSON.stringify({ game_dir: join(fixture, 'minecraft'), isolated: true }),
  );
const child = spawn(executable, [], {
  cwd: root,
  windowsHide: true,
  env: { ...process.env, RUST_HMCL_DATA_DIR: fixture },
  stdio: ['pipe', 'pipe', 'pipe'],
});
const pending = [];
let errorOutput = '';
child.stderr.on('data', (data) => (errorOutput += data));
child.on('error', (error) => {
  while (pending.length) pending.shift().reject(error);
});
child.on('exit', (code) => {
  while (pending.length)
    pending.shift().reject(new Error(`Service exited ${code}: ${errorOutput}`));
});
createInterface({ input: child.stdout }).on('line', (line) => {
  const request = pending.shift();
  if (!request) return;
  try {
    const result = JSON.parse(line);
    result.ok ? request.resolve(result.value) : request.reject(new Error(result.error));
  } catch (error) {
    request.reject(error);
  }
});
const rpc = (method, args = {}) =>
  new Promise((resolve, reject) => {
    pending.push({ resolve, reject });
    child.stdin.write(JSON.stringify({ method, ...args }) + '\n');
  });
async function runDownload() {
  const id = await rpc('start_job', { job: { kind: 'java', major } });
  const deadline = Date.now() + 20 * 60 * 1000;
  let last = '';
  while (Date.now() < deadline) {
    const task = await rpc('task');
    if (task.status !== last) {
      console.log(task.status);
      last = task.status;
    }
    if (task.id === id && !task.running) {
      assert.equal(task.error, null);
      return task;
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
  await rpc('cancel_job');
  throw new Error('Java download did not finish within 20 minutes');
}
async function sha256(path) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest('hex');
}
try {
  await runDownload();
  const snapshot = await rpc('snapshot');
  const path = snapshot.config.java_paths.find((path) => path.includes(`${sep}java${sep}17${sep}`));
  assert.ok(path, 'Downloaded Java path was not registered');
  const registered = await rpc('register_java', { path });
  assert.equal(registered.runtime.major, major);
  assert.equal(registered.runtime.compatible, true);
  const executableTime = (await stat(path)).mtimeMs;
  await runDownload();
  const second = await rpc('snapshot');
  assert.equal(second.config.java_paths.filter((value) => value === path).length, 1);
  assert.equal(
    (await stat(path)).mtimeMs,
    executableTime,
    'A valid runtime should be reused without extraction',
  );
  const downloads = join(fixture, 'downloads');
  const entries = await readdir(downloads, { withFileTypes: true });
  assert.equal(
    entries.filter((entry) => entry.isDirectory() && entry.name.startsWith('java-stage-')).length,
    0,
  );
  const archives = [];
  for (const entry of entries.filter(
    (entry) => entry.isFile() && /\.(zip|tar\.gz)$/.test(entry.name),
  )) {
    const file = join(downloads, entry.name);
    const checksum = await sha256(file);
    assert.ok(
      path.includes(checksum),
      'Installed directory must bind the official archive checksum',
    );
    archives.push({ name: entry.name, bytes: (await stat(file)).size, sha256: checksum });
  }
  assert.ok(archives.length);
  const proof = {
    fixture,
    serviceSha256: await sha256(executable),
    runtime: registered.runtime,
    archives,
    versionProbesOnly: true,
    gameStarted: false,
    desktopInput: false,
    reuseWithoutExtraction: true,
    stagingRemoved: true,
    verifiedAt: new Date().toISOString(),
  };
  await mkdir(join(root, 'dist'), { recursive: true });
  await writeFile(join(root, 'dist', 'java-online-proof.json'), JSON.stringify(proof, null, 2));
  console.log(`Java online verification passed: ${join(root, 'dist', 'java-online-proof.json')}`);
} finally {
  child.stdin.end();
  if (child.exitCode === null) {
    const timeout = setTimeout(() => child.kill(), 3000);
    await once(child, 'exit');
    clearTimeout(timeout);
  }
}
