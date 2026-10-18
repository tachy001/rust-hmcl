// Independent local fixtures. JAR bytes are never executed; the ZIP has only pack.mcmeta.
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
export async function localImportFixtures(fixture) {
  const directory = join(fixture, 'local-import-sources');
  await mkdir(directory, { recursive: true });
  const mod = join(directory, 'imported-sample.jar');
  const removedMod = join(directory, 'not-selected.jar');
  const zip = join(directory, 'Imported ZIP.zip');
  const folder = join(directory, 'Imported Folder');
  await writeFile(mod, 'independent imported mod bytes; never executed');
  await writeFile(removedMod, 'removed selection; never executed');
  await writeFile(
    zip,
    Buffer.from(
      'UEsDBBQAAAAIABlyUl32NE/RRQAAAEAAAAALAAAAcGFjay5tY21ldGGqVipITM5WsqoG0/Fp+UW5iSVKVoamOkopqcXJRZkFJZn5eUpWSp65BflFJakpClGeAQppmRUlpUWpSrW1AAAAAP//AwBQSwECFAAUAAAACAAZclJd9jRP0UUAAABAAAAACwAAAAAAAAAAAAAAAAAAAAAAcGFjay5tY21ldGFQSwUGAAAAAAEAAQA5AAAAbgAAAAAA',
      'base64',
    ),
  );
  await mkdir(join(folder, 'assets', 'empty'), { recursive: true });
  await writeFile(
    join(folder, 'pack.mcmeta'),
    JSON.stringify({ pack: { pack_format: 15, description: 'Imported folder fixture' } }),
  );
  await writeFile(join(folder, 'assets', 'sample.txt'), 'independent resource content');
  return { mod, removedMod, zip, folder };
}
