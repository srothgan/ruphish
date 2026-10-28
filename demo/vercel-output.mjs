import { createHash } from 'node:crypto';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { dirname, extname, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

// Build Output API v3: publish only the manifest-validated static distribution.
// No source tree, environment files, functions, or runtime credentials are copied.
export const routes = [
  {
    src: '^/.*$',
    headers: {
      'X-Robots-Tag': 'noindex, nofollow',
      'Cache-Control': 'public, max-age=0, must-revalidate',
    },
    continue: true,
  },
  { src: '^/(?:api|auth|internal)(?:/.*)?$', status: 404 },
  { src: '^/.*$', methods: ['POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS'], status: 405 },
  { handle: 'filesystem' },
  { src: '^/[^.]*$', methods: ['GET', 'HEAD'], dest: '/index.html' },
  { src: '^/.*$', status: 404 },
];

export async function packageDemo(source, output) {
  const files = new Map();
  async function collect(directory, prefix = '') {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const name = `${prefix}${entry.name}`;
      if (
        !/^[a-zA-Z0-9_/-][a-zA-Z0-9_./-]*$/.test(name) ||
        name.split('/').some((part) => part.startsWith('.'))
      ) {
        throw new Error(`Unsupported asset path: ${name}`);
      }
      if (entry.isDirectory()) {
        await collect(join(directory, entry.name), `${name}/`);
      } else if (entry.isFile()) {
        files.set(name, await readFile(join(directory, entry.name)));
      } else {
        throw new Error(`Unsupported asset type: ${name}`);
      }
    }
  }
  await collect(source);
  const manifest = JSON.parse(files.get('asset-manifest.json')?.toString() ?? 'null');
  if (
    !manifest ||
    Array.isArray(manifest) ||
    typeof manifest !== 'object' ||
    Object.keys(manifest).length + 1 !== files.size
  ) {
    throw new Error('Missing or inconsistent asset manifest');
  }
  const types = {
    '.wasm': 'application/wasm',
    '.js': 'text/javascript; charset=utf-8',
    '.mjs': 'text/javascript; charset=utf-8',
  };
  const allowed = new Set([
    '.html',
    '.js',
    '.mjs',
    '.css',
    '.wasm',
    '.txt',
    '.json',
    '.svg',
    '.png',
    '.jpg',
    '.jpeg',
    '.gif',
    '.ico',
    '.woff',
    '.woff2',
    '.map',
  ]);
  for (const [name, bytes] of files) {
    if (!allowed.has(extname(name)) || /^(api|auth|internal)(\/|$)/.test(name)) {
      throw new Error(`Unsupported asset: ${name}`);
    }
    if (
      name !== 'asset-manifest.json' &&
      manifest[name] !== createHash('sha256').update(bytes).digest('hex')
    ) {
      throw new Error(`Asset integrity failed: ${name}`);
    }
  }
  for (const name of [
    'index.html',
    'visual-frame.js',
    'visual-frame.css',
    'THIRD-PARTY-NOTICES.txt',
  ]) {
    if (!files.has(name)) {
      throw new Error(`Missing required asset: ${name}`);
    }
  }
  if (![...files.keys()].some((name) => name.endsWith('.wasm'))) {
    throw new Error('Missing compiled WASM');
  }

  await mkdir(output, { recursive: true });
  if ((await readdir(output)).length) {
    throw new Error('Output directory must be empty');
  }
  const overrides = {};
  for (const [name, bytes] of files) {
    const target = join(output, 'static', name);
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, bytes, { flag: 'wx' });
    if (types[extname(name)]) {
      overrides[name] = { contentType: types[extname(name)] };
    }
  }
  await writeFile(
    join(output, 'config.json'),
    `${JSON.stringify({ version: 3, routes, overrides }, null, 2)}\n`,
    { flag: 'wx' },
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  await packageDemo(resolve('dist/admin'), resolve('.vercel/output'));
  console.log('Prepared manifest-validated static demo in .vercel/output');
}
