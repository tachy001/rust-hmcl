// Independent .mrpack with optional/server files and client overrides.
// Cached handwritten client/index bytes let the installer run without Java or network.
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
export async function modpackFixture(fixture) {
  const root = join(fixture, 'minecraft');
  const base = join(root, 'versions', 'pack-base');
  await mkdir(base, { recursive: true });
  await mkdir(join(root, 'assets', 'indexes'), { recursive: true });
  const client = Buffer.from('cached fixture client; never launched');
  const assets = Buffer.from('{"objects":{}}');
  const hash = (bytes) => createHash('sha1').update(bytes).digest('hex');
  await writeFile(join(base, 'pack-base.jar'), client);
  await writeFile(join(root, 'assets', 'indexes', 'pack-cache.json'), assets);
  await writeFile(
    join(base, 'pack-base.json'),
    JSON.stringify({
      id: 'pack-base',
      libraries: [],
      downloads: {
        client: { url: 'https://example.invalid/not-requested-client', sha1: hash(client) },
      },
      assetIndex: {
        id: 'pack-cache',
        url: 'https://example.invalid/not-requested-index',
        sha1: hash(assets),
      },
    }),
  );
  const path = join(fixture, 'journey.mrpack');
  const bytes = Buffer.from(
    'UEsDBBQAAAAIAOFzUl0fsSnM+AAAACICAAATAAAAbW9kcmludGguaW5kZXguanNvbsSPMW7DMAxFrxL8WbGTVVvH9ABdigysRFdqLUqhZLdB4LsXRhAgATp0K0d+ku/xgiFrovbCWmMW2L3BOyWGRYrCTmloMJBr66k21s1znlT4DIP5unXwsNh3u24HgzqlRHqGxUE8FxbP0jaOXGC/GeJ3m5RhcItc5Ap7uaNZFHKf2zeqjMVgiOM68XpBoRZWr+xrn0uLWWjsPkhhEKiG650aaA+L3R9rJfj8JWMmv1IQWivV9r3z0qXsNUoLncupF55Zt8qniWtjv30wOBqwzCvfjZFlfeKWY1nMo3tlnVn/z/yO/4v3JHUqJWtjj2U5Lj8AAAD//wMAUEsDBBQAAAAIAOFzUl12zSnIFwAAAA8AAAAaAAAAb3ZlcnJpZGVzL2NvbmZpZy9hc3Rlci50eHRKzs/Nzc9TyC9LLSrKTEkFAAAA//8DAFBLAwQUAAAACADhc1JdWLiegRwAAAAUAAAAIQAAAGNsaWVudC1vdmVycmlkZXMvY29uZmlnL2FzdGVyLnR4dErOyUzNK1HIL0stKspMSVUoz8wrBgAAAP//AwBQSwECFAAUAAAACADhc1JdH7EpzPgAAAAiAgAAEwAAAAAAAAAAAAAAAAAAAAAAbW9kcmludGguaW5kZXguanNvblBLAQIUABQAAAAIAOFzUl12zSnIFwAAAA8AAAAaAAAAAAAAAAAAAAAAACkBAABvdmVycmlkZXMvY29uZmlnL2FzdGVyLnR4dFBLAQIUABQAAAAIAOFzUl1YuJ6BHAAAABQAAAAhAAAAAAAAAAAAAAAAAHgBAABjbGllbnQtb3ZlcnJpZGVzL2NvbmZpZy9hc3Rlci50eHRQSwUGAAAAAAMAAwDYAAAA0wEAAAAA',
    'base64',
  );
  await writeFile(path, bytes);
  return { path, sha1: hash(bytes), client };
}
