// Opt-in official Minecraft/Fabric/Quilt download verification. Never starts Java or a game.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { once } from 'node:events';
import { mkdtemp, mkdir, copyFile, readFile, writeFile, realpath, stat } from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve, join, sep, delimiter } from 'node:path';
import assert from 'node:assert/strict';
const root = resolve(import.meta.dirname, '..');
const tools = join(root, '.tools');
await mkdir(tools, { recursive: true });
const reuse = process.argv.indexOf('--fixture');
const fixture =
  reuse < 0
    ? await mkdtemp(join(tools, 'install-online-proof-'))
    : await realpath(resolve(process.argv[reuse + 1]));
assert.ok(
  fixture.startsWith((await realpath(tools)) + sep) &&
    /install-online-proof-[a-z0-9]+$/i.test(fixture),
);
const game = join(fixture, 'minecraft');
const version = '1.20.1';
if (reuse < 0)
  await writeFile(
    join(fixture, 'config.json'),
    JSON.stringify({ game_dir: game, isolated: true, player: 'InstallProof', auto_java: false }),
  );
const source =
  process.env.HMCL_SERVICE_BIN ??
  join(
    root,
    '..',
    'target',
    'debug',
    process.platform === 'win32' ? 'hmcl-service.exe' : 'hmcl-service',
  );
const executable = join(fixture, process.platform === 'win32' ? 'service.exe' : 'service');
await copyFile(source, executable);
const child = spawn(executable, [], {
  cwd: root,
  windowsHide: true,
  env: { ...process.env, RUST_HMCL_DATA_DIR: fixture },
  stdio: ['pipe', 'pipe', 'pipe'],
});
const pending = [];
let stderr = '';
child.stderr.on('data', (data) => {
  stderr += data;
});
child.on('error', (error) => {
  while (pending.length) pending.shift().reject(error);
});
child.on('exit', (code) => {
  while (pending.length) pending.shift().reject(new Error(`Service exited ${code}: ${stderr}`));
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
function rpc(method, args = {}) {
  assert.ok(
    [
      'snapshot',
      'versions',
      'start_job',
      'task',
      'cancel_job',
      'launch_plan',
      'mod_environment',
    ].includes(method),
  );
  if (method === 'start_job')
    assert.ok(
      ['install', 'repair', 'fabric', 'quilt', 'duplicate', 'rename'].includes(args.job.kind),
      'This verification never launches Java or games',
    );
  return new Promise((resolve, reject) => {
    pending.push({ resolve, reject });
    child.stdin.write(JSON.stringify({ method, ...args }) + '\n');
  });
}
async function digest(path, algorithm = 'sha1') {
  const hash = createHash(algorithm);
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest('hex');
}
async function run(job, expectedFailure = false) {
  const id = await rpc('start_job', { job });
  const deadline = Date.now() + 40 * 60 * 1000;
  let printed = 0;
  let last = '';
  while (Date.now() < deadline) {
    const task = await rpc('task');
    const phase = task.status.replace(/\d+(?:\.\d+)?/g, '#');
    if (phase !== last || Date.now() - printed > 10000 || !task.running) {
      console.log(`${job.kind}: ${task.status}`);
      printed = Date.now();
      last = phase;
    }
    assert.equal(task.game_running, false);
    if (task.id === id && !task.running) {
      if (expectedFailure) assert.match(task.error ?? '', /同名实例已存在/);
      else assert.equal(task.error, null);
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
  await rpc('cancel_job');
  throw new Error(`Install deadline exceeded: ${job.kind}`);
}
async function validatePlan(id, mainClass) {
  const args = await rpc('launch_plan', { id });
  assert.ok(args.includes(mainClass));
  assert.ok(args.every((value) => !value.includes('${')));
  const cp = args[args.indexOf('-cp') + 1];
  assert.ok(args.includes('-cp') && cp);
  const files = cp.split(delimiter);
  assert.ok(files.length > 20);
  for (const path of files)
    assert.ok((await stat(path)).isFile(), `Missing classpath entry: ${path}`);
  return { id, mainClass, classpathFiles: files.length, unresolvedPlaceholders: false };
}
try {
  console.log(`Isolated fixture: ${fixture}`);
  const manifest = await rpc('versions');
  const entry = manifest.find((item) => item.id === version);
  assert.ok(entry?.sha1);
  await run({ kind: 'install', id: version });
  const metadataPath = join(game, 'versions', version, `${version}.json`);
  assert.equal(await digest(metadataPath), entry.sha1);
  const meta = JSON.parse(await readFile(metadataPath, 'utf8'));
  const jar = join(game, 'versions', version, `${version}.jar`);
  assert.equal(await digest(jar), meta.downloads.client.sha1);
  let clientMtime = (await stat(jar)).mtimeMs;
  const assetIndex = join(game, 'assets', 'indexes', `${meta.assetIndex.id}.json`);
  assert.equal(await digest(assetIndex), meta.assetIndex.sha1);
  const assets = JSON.parse(await readFile(assetIndex, 'utf8'));
  const objects = [
    ...new Map(Object.values(assets.objects).map((item) => [item.hash, item])).values(),
  ];
  let next = 0;
  let assetBytes = 0;
  await Promise.all(
    Array.from({ length: 8 }, async () => {
      for (;;) {
        const object = objects[next++];
        if (!object) return;
        const path = join(game, 'assets', 'objects', object.hash.slice(0, 2), object.hash);
        assert.equal(await digest(path), object.hash);
        const size = (await stat(path)).size;
        assert.equal(size, object.size);
        assetBytes += size;
      }
    }),
  );
  const plans = [await validatePlan(version, 'net.minecraft.client.main.Main')];
  const libraryArtifacts = meta.libraries
    .flatMap((library) => [
      library.downloads?.artifact,
      ...Object.values(library.downloads?.classifiers ?? {}),
    ])
    .filter((item) => item?.path && item.sha1);
  const verifiedLibraries = [];
  for (const artifact of libraryArtifacts) {
    const path = join(game, 'libraries', artifact.path);
    const info = await stat(path).catch((error) => {
      if (error.code !== 'ENOENT') throw error;
      return null;
    });
    if (!info) continue; // Libraries for other platforms are not installed on this host.
    assert.equal(await digest(path), artifact.sha1);
    if (artifact.size !== undefined) assert.equal(info.size, artifact.size);
    verifiedLibraries.push({ path: artifact.path, sha1: artifact.sha1, bytes: info.size });
  }
  assert.ok(verifiedLibraries.length > 20);
  await run({ kind: 'repair', id: version });
  assert.equal((await stat(jar)).mtimeMs, clientMtime, 'Verified client should be reused');
  const repairCorruption = process.argv.includes('--repair-corruption');
  if (repairCorruption) {
    const retainedAsset = join(
      game,
      'assets',
      'objects',
      objects[0].hash.slice(0, 2),
      objects[0].hash,
    );
    const assetMtime = (await stat(retainedAsset)).mtimeMs;
    await writeFile(jar, 'intentional corruption of the isolated proof client only');
    await run({ kind: 'repair', id: version });
    assert.equal(await digest(jar), meta.downloads.client.sha1);
    assert.equal(
      (await stat(retainedAsset)).mtimeMs,
      assetMtime,
      'Valid assets should not be downloaded again',
    );
    clientMtime = (await stat(jar)).mtimeMs;
  }
  const copied = 'Online Vanilla Copy';
  const renamed = 'Online Vanilla Renamed';
  const existing = await rpc('snapshot');
  if (!existing.instances.some((item) => item.id === renamed)) {
    if (!existing.instances.some((item) => item.id === copied))
      await run({ kind: 'duplicate', id: version, name: copied, copy_saves: false });
    await run({ kind: 'rename', id: copied, name: renamed });
  }
  const renamedMetadata = JSON.parse(
    await readFile(join(game, 'versions', renamed, `${renamed}.json`), 'utf8'),
  );
  assert.equal(renamedMetadata.rustHmclGameVersion, version);
  plans.push(await validatePlan(renamed, 'net.minecraft.client.main.Main'));
  for (const loader of ['fabric', 'quilt']) {
    const name = loader === 'fabric' ? 'Online Fabric Instance' : 'Online Quilt Instance';
    const current = await rpc('snapshot');
    if (!current.instances.some((item) => item.id === name))
      await run({ kind: loader, id: renamed, name });
    const snapshot = await rpc('snapshot');
    const id = name;
    assert.ok(snapshot.instances.some((item) => item.id === id));
    assert.notEqual(id, version);
    const profile = JSON.parse(await readFile(join(game, 'versions', id, `${id}.json`), 'utf8'));
    assert.equal(profile.inheritsFrom, renamed);
    const mainClass =
      loader === 'fabric'
        ? 'net.fabricmc.loader.impl.launch.knot.KnotClient'
        : 'org.quiltmc.loader.impl.launch.knot.KnotClient';
    plans.push(await validatePlan(id, mainClass));
    assert.deepEqual(await rpc('mod_environment', { id }), { game: version, loader });
    assert.equal(
      (await stat(jar)).mtimeMs,
      clientMtime,
      'Loader install must reuse the base client',
    );
    const before = await readFile(join(game, 'versions', id, `${id}.json`));
    await assert.rejects(rpc('start_job', { job: { kind: loader, id: renamed, name } }));
    assert.deepEqual(await readFile(join(game, 'versions', id, `${id}.json`)), before);
    const defaultInstance = snapshot.instances.find(
      (item) => item.icon === loader && item.id !== name,
    );
    if (defaultInstance) {
      const defaultPath = join(game, 'versions', defaultInstance.id, `${defaultInstance.id}.json`);
      const defaultBefore = await readFile(defaultPath);
      await run({ kind: loader, id: version }, true);
      assert.deepEqual(await readFile(defaultPath), defaultBefore);
    }
  }
  await mkdir(join(root, 'dist'), { recursive: true });
  await writeFile(
    join(root, 'dist', 'install-online-proof.json'),
    JSON.stringify(
      {
        fixture,
        reusedFixture: reuse >= 0,
        serviceSha256: await digest(executable, 'sha256'),
        version,
        metadataSha1: entry.sha1,
        clientSha1: meta.downloads.client.sha1,
        uniqueAssetFiles: objects.length,
        verifiedAssetBytes: assetBytes,
        verifiedLibraries,
        renamedGameVersion: renamedMetadata.rustHmclGameVersion,
        existingLoaderProfilesPreserved: true,
        corruptedClientRepaired: repairCorruption,
        plans,
        verifiedClientReused: true,
        javaOrGameStarted: false,
        desktopInput: false,
        verifiedAt: new Date().toISOString(),
        limitations:
          'Files and read-only argument plans only; this does not prove the game window, rendering or authentication.',
      },
      null,
      2,
    ),
  );
  console.log('Official game and loader installation verification passed.');
} finally {
  child.stdin.end();
  if (child.exitCode === null) {
    const timeout = setTimeout(() => child.kill(), 3000);
    await once(child, 'exit');
    clearTimeout(timeout);
  }
}
