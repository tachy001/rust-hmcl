// Real bundled WebView + normal IPC on each host. Hidden window; no desktop input or Java.
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, writeFile, readFile, realpath, access } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { resolve, join } from 'node:path';
import assert from 'node:assert/strict';

const root = resolve(import.meta.dirname, '..');
const platform = { win32: 'windows', darwin: 'macos', linux: 'linux' }[process.platform];
assert.ok(platform, 'Unsupported desktop platform');
const executable = await realpath(
  resolve(
    process.env.HMCL_DESKTOP_BIN ??
      join(
        root,
        'target',
        'release',
        process.platform === 'win32' ? 'rust-hmcl-desktop.exe' : 'rust-hmcl-desktop',
      ),
  ),
);
await mkdir(join(root, '.tools'), { recursive: true });
const fixture = await mkdtemp(join(root, '.tools', 'platform-ui-proof-'));
await writeFile(
  join(fixture, 'config.json'),
  JSON.stringify({
    game_dir: join(fixture, 'minecraft'),
    player: 'PlatformProof',
    auto_java: false,
  }),
);
const output = join(root, 'dist', 'platform-ui-proof', `${process.platform}-${process.arch}`);
await mkdir(output, { recursive: true });
const child = spawn(executable, ['--verify-platform'], {
  cwd: root,
  windowsHide: true,
  stdio: ['ignore', 'pipe', 'pipe'],
  env: { ...process.env, RUST_HMCL_DATA_DIR: fixture },
});
let logs = '';
for (const stream of [child.stdout, child.stderr])
  stream.on('data', (data) => {
    logs = (logs + data).slice(-131072);
  });
const deadline = setTimeout(() => child.kill('SIGKILL'), 120000);
let code;
try {
  [code] = await once(child, 'exit');
} finally {
  clearTimeout(deadline);
  await writeFile(join(output, 'runtime.log'), logs);
}
const config = JSON.parse(await readFile(join(fixture, 'config.json'), 'utf8'));
assert.equal(
  code,
  0,
  `Native startup/IPC failed: ${decodeURIComponent(config.extra_jvm ?? '')}\n${logs}`,
);
assert.equal(config.player, 'IPCReady');
assert.equal(config.auto_java, false);
await assert.rejects(
  access(join(fixture, 'minecraft')),
  'Verification must not create or install a game',
);
const native = JSON.parse(await readFile(join(fixture, 'native-window.json'), 'utf8'));
assert.equal(native.visible, false);
assert.equal(native.platform.system, platform);
assert.equal(
  native.platform.architecture,
  { x64: 'x86_64', arm64: 'aarch64', ia32: 'x86' }[process.arch],
);
assert.equal(native.platform.java_executable, process.platform === 'win32' ? 'java.exe' : 'java');
assert.equal(native.platform.classpath_separator, process.platform === 'win32' ? ';' : ':');
assert.equal(resolve(native.platform.data_directory), resolve(fixture));
const proof = {
  platform,
  architecture: process.arch,
  fixture,
  executable,
  sourceRevision: execFileSync('git', ['rev-parse', 'HEAD'], {
    cwd: root,
    encoding: 'utf8',
  }).trim(),
  executableSha256: createHash('sha256')
    .update(await readFile(executable))
    .digest('hex'),
  passed: [
    'real native hidden window',
    'bundled Vue application mounted',
    'bundled SVG decoded',
    'normal snapshot and platform IPC',
    'native OS and architecture match the runner',
    'configuration write/read round trip',
    'browser context menu suppressed',
    'no game directory created',
    'clean application exit',
  ],
  desktopInput: false,
  javaOrGameStarted: false,
  native,
  limitations: [
    'No visible compositor or physical input verification',
    'No game rendering, authentication or game startup',
  ],
};
await writeFile(join(output, 'proof.json'), JSON.stringify(proof, null, 2));
console.log(
  `Native platform verification passed: ${platform}/${process.arch} (${proof.passed.length} checks)`,
);
