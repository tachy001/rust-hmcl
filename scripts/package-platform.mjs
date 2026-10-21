// Host-native distribution packaging; Unix tar preserves executable modes and bundle links.
import { execFileSync } from 'node:child_process';
import { mkdir, readFile, writeFile, cp, chmod, readdir, stat } from 'node:fs/promises';
import { resolve, join, basename } from 'node:path';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
const root = resolve(import.meta.dirname, '..');
const os = { win32: 'windows', darwin: 'macos', linux: 'linux' }[process.platform];
assert.ok(os && ['x64', 'arm64'].includes(process.arch));
const revision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim();
assert.equal(
  execFileSync('git', ['status', '--porcelain'], { cwd: root, encoding: 'utf8' }).trim(),
  '',
  'Package from a clean source tree',
);
const short = revision.slice(0, 7);
const output = join(root, 'dist', 'distributions');
await mkdir(output, { recursive: true });
if (process.platform === 'win32') {
  assert.equal(process.arch, 'x64', 'Windows single EXE distribution currently targets x64');
  const binary = join(root, 'target/release/rust-hmcl-desktop.exe');
  const proof = JSON.parse(
    await readFile(
      join(root, 'dist/platform-ui-proof', `${process.platform}-${process.arch}`, 'proof.json'),
      'utf8',
    ),
  );
  assert.equal(
    proof.executableSha256,
    createHash('sha256')
      .update(await readFile(binary))
      .digest('hex'),
  );
  execFileSync(
    'pwsh',
    [
      '-NoProfile',
      '-File',
      join(root, 'scripts', 'package-windows.ps1'),
      '-TargetDir',
      'target',
      '-OutputName',
      `aster-windows-${process.arch}-${short}`,
      '-BinaryRevision',
      revision,
    ],
    { cwd: root, stdio: 'inherit' },
  );
  await cp(
    join(root, 'dist', `aster-launcher-windows-x64-${short}.zip`),
    join(output, `aster-launcher-windows-${process.arch}-${short}.zip`),
  );
} else {
  const stage = join(root, '.tools', `distribution-${os}-${process.arch}-${short}`);
  await mkdir(stage, { recursive: true });
  const app = join(root, 'target/release/bundle/macos/Aster Launcher.app');
  const binary =
    process.platform === 'darwin'
      ? join(app, 'Contents/MacOS/rust-hmcl-desktop')
      : join(root, 'target/release/rust-hmcl-desktop');
  const proof = JSON.parse(
    await readFile(
      join(root, 'dist/platform-ui-proof', `${process.platform}-${process.arch}`, 'proof.json'),
      'utf8',
    ),
  );
  const sha256 = createHash('sha256')
    .update(await readFile(binary))
    .digest('hex');
  assert.equal(
    proof.executableSha256,
    sha256,
    'Package the same executable that passed native verification',
  );
  if (process.platform === 'darwin') {
    execFileSync('codesign', ['--verify', '--deep', '--strict', app], { stdio: 'inherit' });
    await cp(app, join(stage, basename(app)), { recursive: true, verbatimSymlinks: true });
    assert.ok(
      (await stat(join(stage, basename(app), 'Contents/MacOS/rust-hmcl-desktop'))).mode & 0o111,
    );
  } else {
    await cp(binary, join(stage, 'AsterLauncher'));
    await chmod(join(stage, 'AsterLauncher'), 0o755);
    const debs = (await readdir(join(root, 'target/release/bundle/deb'))).filter((file) =>
      file.endsWith('.deb'),
    );
    assert.equal(debs.length, 1);
    const unpacked = `${stage}-deb-verification`;
    execFileSync('dpkg-deb', ['-x', join(root, 'target/release/bundle/deb', debs[0]), unpacked]);
    const installed = join(unpacked, 'usr/bin/rust-hmcl-desktop');
    assert.equal(
      createHash('sha256')
        .update(await readFile(installed))
        .digest('hex'),
      sha256,
      'Debian package must contain the tested executable',
    );
    assert.ok((await stat(installed)).mode & 0o111);
    await cp(
      join(root, 'target/release/bundle/deb', debs[0]),
      join(output, `aster-launcher-linux-${process.arch}-${short}.deb`),
    );
  }
  for (const [source, name] of [
    ['LICENSE', 'LICENSE'],
    ['docs/PLATFORMS.md', 'README.md'],
    ['assets/aster/ATTRIBUTION.md', 'ASTER-ATTRIBUTION.md'],
    ['assets/hmcl/ATTRIBUTION.md', 'HMCL-ATTRIBUTION.md'],
  ])
    await cp(join(root, source), join(stage, name));
  await writeFile(
    join(stage, 'build-info.json'),
    JSON.stringify(
      {
        product: 'Aster Launcher',
        source_revision: revision,
        source_dirty: false,
        platform: os,
        architecture: process.arch,
        executable_sha256: sha256,
        preview: true,
      },
      null,
      2,
    ),
  );
  const archive = join(output, `aster-launcher-${os}-${process.arch}-${short}.tar.gz`);
  execFileSync('tar', ['-czf', archive, '-C', stage, '.'], { cwd: root, stdio: 'inherit' });
}
const files = await readdir(output);
const checksums = await Promise.all(
  files
    .filter((file) => file.includes(short))
    .map(async (name) => ({
      name,
      sha256: createHash('sha256')
        .update(await readFile(join(output, name)))
        .digest('hex'),
    })),
);
await writeFile(
  join(output, `SHA256SUMS-${os}-${process.arch}.txt`),
  checksums.map((file) => `${file.sha256}  ${file.name}`).join('\n') + '\n',
);
console.log(`Distribution ready: ${os}/${process.arch} ${revision}`);
