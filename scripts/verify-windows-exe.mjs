// Inspect the PE import table without depending on Visual Studio or GNU utilities.
import { readFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import assert from 'node:assert/strict';

export async function verifyWindowsExe(path) {
  const bytes = await readFile(path);
  assert.equal(bytes.toString('ascii', 0, 2), 'MZ', 'Expected a Windows executable');
  const pe = bytes.readUInt32LE(0x3c);
  assert.equal(bytes.toString('ascii', pe, pe + 4), 'PE\0\0', 'Invalid PE signature');
  const sectionCount = bytes.readUInt16LE(pe + 6);
  const optional = pe + 24;
  const magic = bytes.readUInt16LE(optional);
  assert.ok(magic === 0x20b || magic === 0x10b, 'Unsupported PE optional header');
  const directories = optional + (magic === 0x20b ? 112 : 96);
  const sections = optional + bytes.readUInt16LE(pe + 20);
  function offset(rva) {
    for (let i = 0; i < sectionCount; i++) {
      const section = sections + i * 40;
      const start = bytes.readUInt32LE(section + 12);
      const size = bytes.readUInt32LE(section + 16);
      if (rva >= start && rva < start + size) return bytes.readUInt32LE(section + 20) + rva - start;
    }
    throw new Error(`PE address outside file sections: ${rva}`);
  }
  function nameAt(rva) {
    const start = offset(rva);
    const end = bytes.indexOf(0, start);
    assert.ok(end >= start, 'Unterminated DLL name');
    return bytes.toString('ascii', start, end);
  }
  const imports = [];
  const ordinary = bytes.readUInt32LE(directories + 8);
  if (ordinary) {
    for (let entry = offset(ordinary); bytes.readUInt32LE(entry + 12); entry += 20)
      imports.push(nameAt(bytes.readUInt32LE(entry + 12)));
  }
  const delayed = bytes.readUInt32LE(directories + 13 * 8);
  if (delayed) {
    for (let entry = offset(delayed); bytes.readUInt32LE(entry + 4); entry += 32) {
      assert.equal(bytes.readUInt32LE(entry) & 1, 1, 'Expected RVA based delay imports');
      imports.push(nameAt(bytes.readUInt32LE(entry + 4)));
    }
  }
  assert.ok(imports.length, 'No PE imports found');
  assert.ok(
    !imports.some((name) => /webview2loader/i.test(name)),
    'EXE still imports WebView2Loader.dll; build with Windows MSVC for static linking',
  );
  return imports;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const imports = await verifyWindowsExe(process.argv[2]);
  console.log(`Verified: no external WebView2 loader (${imports.length} system imports).`);
}
