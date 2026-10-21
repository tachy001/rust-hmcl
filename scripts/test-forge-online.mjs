// Opt-in execution of the official Forge installer, plus Java version probes. No game or desktop input.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { once } from 'node:events';
import {
  mkdtemp,
  mkdir,
  copyFile,
  readFile,
  writeFile,
  realpath,
  stat,
  readdir,
} from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve, join, dirname, basename, sep, delimiter } from 'node:path';
import assert from 'node:assert/strict';
const root = resolve(import.meta.dirname, '..');
const tools = join(root, '.tools');
await mkdir(tools, { recursive: true });
const value = (flag) => {
  const index = process.argv.indexOf(flag);
  return index < 0 ? null : process.argv[index + 1];
};
const neo = process.argv.includes('--neoforge');
const loader = neo ? 'neoforge' : 'forge';
const label = neo ? 'NeoForge' : 'Forge';
const minecraft = neo ? '1.20.4' : '1.20.1';
const loaderVersion = neo ? '20.4.251' : '47.4.10';
const recordKey = neo ? 'rustHmclNeoForge' : 'rustHmclForge';
const reuse = value('--fixture');
const fixture = reuse
  ? await realpath(resolve(reuse))
  : await mkdtemp(join(tools, `${loader}-online-proof-`));
assert.ok(
  fixture.startsWith((await realpath(tools)) + sep) &&
    new RegExp(`^${loader}-online-proof-[a-z0-9]+$`, 'i').test(basename(fixture)),
);
if (!reuse) {
  assert.ok(
    value('--game-dir') && value('--java'),
    'Pass an isolated --game-dir and --java executable.',
  );
  const game = await realpath(resolve(value('--game-dir')));
  assert.ok(
    game.startsWith((await realpath(tools)) + sep) &&
      /^install-online-proof-[a-z0-9]+$/i.test(basename(dirname(game))),
  );
  await writeFile(
    join(fixture, 'config.json'),
    JSON.stringify({
      game_dir: game,
      java: await realpath(value('--java')),
      auto_java: false,
      isolated: true,
      player: 'ForgeProof',
    }),
  );
}
const config = JSON.parse(await readFile(join(fixture, 'config.json'), 'utf8'));
const game = await realpath(config.game_dir);
assert.ok(
  game.startsWith((await realpath(tools)) + sep) &&
    /^install-online-proof-[a-z0-9]+$/i.test(basename(dirname(game))),
  'Reused game directory must remain inside an owned install fixture',
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
  if (method === 'start_job')
    assert.ok(
      [loader, 'repair', 'install_mod'].includes(args.job.kind),
      'This script never launches a game',
    );
  if (method === 'start_job' && args.job.kind === 'install_mod')
    assert.ok(
      args.job.project === 'jei' &&
        args.job.id === `${label}-Proof-${basename(fixture).split('-').at(-1)}`,
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
async function run(job) {
  const id = await rpc('start_job', { job });
  const deadline = Date.now() + 20 * 60 * 1000;
  let printed = 0,
    last = '';
  while (Date.now() < deadline) {
    const task = await rpc('task');
    assert.equal(task.game_running, false);
    if (task.status !== last || Date.now() - printed > 10000 || !task.running) {
      console.log(`${job.kind}: ${task.status} ${task.logs.at(-1)?.slice(0, 140) ?? ''}`);
      last = task.status;
      printed = Date.now();
    }
    if (task.id === id && !task.running) {
      assert.equal(task.error, null, task.logs.slice(-10).join('\n'));
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
  await rpc('cancel_job');
  throw new Error(`${label} integration deadline exceeded`);
}
try {
  console.log(`Isolated ${label} fixture: ${fixture}`);
  const base = 'Online Vanilla Renamed';
  const baseJson = join(game, 'versions', base, `${base}.json`);
  const baseJar = join(game, 'versions', base, `${base}.jar`);
  const baseBefore = await readFile(baseJson);
  const baseHash = await digest(baseJar);
  const launcherProfiles = join(game, 'launcher_profiles.json');
  const beforeProfiles = await readFile(launcherProfiles).catch((error) => {
    if (error.code !== 'ENOENT') throw error;
    return null;
  });
  const id = `${label}-Proof-${basename(fixture).split('-').at(-1)}`;
  const existing = await rpc('snapshot');
  if (!existing.instances.some((item) => item.id === id))
    await run({ kind: loader, id: base, name: id, version: loaderVersion });
  const path = join(game, 'versions', id, `${id}.json`);
  const profile = JSON.parse(await readFile(path, 'utf8'));
  assert.equal(profile.inheritsFrom, base);
  const installation = profile[recordKey];
  assert.equal(installation.version, loaderVersion);
  assert.deepEqual(await rpc('mod_environment', { id }), { game: minecraft, loader });
  assert.ok(installation.artifacts.length > 30);
  assert.equal((await rpc('snapshot')).config.version_settings[id].isolated, true);
  const plan = await rpc('launch_plan', { id });
  assert.ok(plan.includes('cpw.mods.bootstraplauncher.BootstrapLauncher'));
  assert.ok(plan.includes('forgeclient'));
  assert.ok(plan.every((arg) => !arg.includes('${')));
  for (const flag of ['-cp', '-p']) {
    assert.ok(plan.includes(flag));
    for (const file of plan[plan.indexOf(flag) + 1].split(delimiter))
      assert.ok((await stat(file)).isFile());
  }
  // --dry-run creates the JVM and checks module/main-class loading, without executing main.
  const validationEnv = { ...process.env };
  for (const key of ['JDK_JAVA_OPTIONS', 'JAVA_TOOL_OPTIONS', '_JAVA_OPTIONS'])
    delete validationEnv[key];
  const validation = spawn(config.java, ['--dry-run', ...plan], {
    cwd: fixture,
    windowsHide: true,
    timeout: 30000,
    env: validationEnv,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let validationOutput = '';
  for (const stream of [validation.stdout, validation.stderr])
    stream.on('data', (data) => {
      validationOutput = (validationOutput + data).slice(-65536);
    });
  const [validationCode] = await once(validation, 'exit');
  assert.equal(validationCode, 0, validationOutput);
  for (const artifact of installation.artifacts)
    assert.equal(await digest(join(game, 'libraries', artifact.path)), artifact.sha1);
  const profileBefore = await readFile(path);
  await assert.rejects(
    rpc('start_job', { job: { kind: loader, id: base, name: id, version: loaderVersion } }),
  );
  assert.deepEqual(await readFile(path), profileBefore);
  let repaired = false;
  if (process.argv.includes('--repair-corruption')) {
    const generated = installation.artifacts.find(
      (item) =>
        item.path.startsWith(neo ? 'net/neoforged/neoforge/' : 'net/minecraftforge/forge/') &&
        item.path.endsWith('-client.jar'),
    );
    assert.ok(generated);
    assert.ok(
      (await realpath(join(game, 'libraries', generated.path))).startsWith(game + sep),
      'Corruption check must remain inside the isolated game directory',
    );
    await writeFile(
      join(game, 'libraries', generated.path),
      'deliberately damaged generated Forge artifact inside the isolated fixture',
    );
    await run({ kind: 'repair', id });
    assert.equal(await digest(join(game, 'libraries', generated.path)), generated.sha1);
    assert.deepEqual(await readFile(path), profileBefore);
    repaired = true;
  }
  assert.deepEqual(await readFile(baseJson), baseBefore);
  assert.equal(await digest(baseJar), baseHash);
  const afterProfiles = await readFile(launcherProfiles).catch((error) => {
    if (error.code !== 'ENOENT') throw error;
    return null;
  });
  assert.deepEqual(afterProfiles, beforeProfiles);
  let installerCancellationVerified = false;
  if (process.argv.includes('--check-cancel')) {
    const taskId = await rpc('start_job', { job: { kind: 'repair', id } });
    const deadline = Date.now() + 120000;
    let installerPid;
    while (Date.now() < deadline) {
      const task = await rpc('task');
      const marker = task.logs.find((line) =>
        new RegExp(`^${label} installer PID: \\d+$`).test(line),
      );
      if (task.id === taskId && task.running && marker) {
        installerPid = Number(marker.split(': ')[1]);
        process.kill(installerPid, 0);
        break;
      }
      if (!task.running) {
        await writeFile(
          join(fixture, 'cancellation-last-task.json'),
          JSON.stringify(task, null, 2),
        );
        throw new Error(
          `Installer ended before cancellation check: ${task.error ?? task.status}\n${task.logs.slice(-8).join('\n')}`,
        );
      }
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert.ok(installerPid, 'Installer PID did not appear');
    await rpc('cancel_job');
    const stopDeadline = Date.now() + 15000;
    let stopped = false;
    while (Date.now() < stopDeadline) {
      const task = await rpc('task');
      if (task.id === taskId && !task.running) {
        assert.equal(task.status, '任务已停止');
        stopped = true;
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert.ok(stopped, 'Cancellation did not finish');
    assert.throws(
      () => process.kill(installerPid, 0),
      (error) => error.code === 'ESRCH',
    );
    assert.deepEqual(await readFile(path), profileBefore);
    installerCancellationVerified = true;
  }
  let modSearchVerified = false;
  let modInstallVerified = null;
  if (process.argv.includes('--search') || process.argv.includes('--install-mod')) {
    const projects = await rpc('search_mods', { id, query: 'Just Enough Items' });
    const project = projects.find((item) => item.slug === 'jei');
    assert.ok(project);
    modSearchVerified = true;
    if (process.argv.includes('--install-mod')) {
      await run({ kind: 'install_mod', id, project: 'jei' });
      const mods = join(game, 'versions', id, 'game', 'mods');
      const registry = JSON.parse(await readFile(join(mods, '.rust-modrinth.json'), 'utf8'));
      const record = registry[project.project_id];
      assert.match(record.version, /^[a-zA-Z0-9]+$/);
      const response = await fetch(`https://api.modrinth.com/v2/version/${record.version}`, {
        signal: AbortSignal.timeout(30000),
      });
      assert.equal(response.status, 200);
      const metadata = await response.json();
      assert.ok(metadata.game_versions.includes(minecraft) && metadata.loaders.includes(loader));
      const file = metadata.files.find((file) => file.filename === record.filename);
      assert.ok(file);
      assert.equal(await digest(join(mods, file.filename)), file.hashes.sha1);
      modInstallVerified = {
        filename: file.filename,
        bytes: (await stat(join(mods, file.filename))).size,
        sha1: file.hashes.sha1,
      };
    }
  }
  assert.ok(
    !(await readdir(join(fixture, 'downloads'))).some((name) =>
      name.startsWith(`${loader}-stage-`),
    ),
  );
  await mkdir(join(root, 'dist'), { recursive: true });
  await writeFile(
    join(root, 'dist', `${loader}-online-proof.json`),
    JSON.stringify(
      {
        fixture,
        serviceSha256: await digest(executable, 'sha256'),
        id,
        loader,
        loaderVersion,
        minecraft,
        installerSha1: installation.installer_sha1,
        verifiedArtifacts: installation.artifacts.length,
        baseJarSha1: baseHash,
        basePreserved: true,
        launcherProfilesPreserved: true,
        existingInstancePreserved: true,
        generatedArtifactRepair: repaired,
        installerCancellationVerified,
        modSearchVerified,
        modInstallVerified,
        stageRemoved: true,
        javaInstallerAndVersionProbes: true,
        jvmDryRunWithoutMain: true,
        gameStarted: false,
        desktopInput: false,
        verifiedAt: new Date().toISOString(),
      },
      null,
      2,
    ),
  );
  console.log(`Official ${label} integration verification passed.`);
} finally {
  child.stdin.end();
  if (child.exitCode === null) {
    const timeout = setTimeout(() => child.kill(), 3000);
    await once(child, 'exit');
    clearTimeout(timeout);
  }
}
