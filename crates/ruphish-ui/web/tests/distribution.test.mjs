import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { build } from 'esbuild';

const execute = promisify(execFile);
const generator = fileURLToPath(new URL('../build.mjs', import.meta.url));

test('admin stylesheet bundles the theme palette and visibility rule without import warnings', async () => {
  const result = await build({
    entryPoints: [fileURLToPath(new URL('../../ruphish.css', import.meta.url))],
    bundle: true,
    minify: true,
    external: ['fonts/*'],
    write: false,
    logLevel: 'silent',
  });
  assert.deepEqual(result.warnings, [], 'stylesheet imports must not be silently dropped');
  const css = result.outputFiles[0].text;
  for (const token of ['--ink', '--canvas', '--paper', '--teal-600', '--nav-bg', '--nav-fg']) {
    assert.match(css, new RegExp(`${token}:`), `${token} must be defined, not just referenced`);
  }
  assert.match(css, /\[hidden\]\{display:none!important\}/);
  assert.doesNotMatch(css, /@import\b/, 'the admin stylesheet must contain its theme');
});

test('completed distribution can be promoted immediately and retains nested assets and exact hashes', async () => {
  const temporary = await mkdtemp(path.join(tmpdir(), 'ruphish-distribution-'));
  const staging = path.join(temporary, '.stage');
  const served = path.join(temporary, 'admin');
  const assets = new Map([
    ['index.html', Buffer.from('<!doctype html><title>Distribution fixture</title>')],
    ['fonts/fixture.woff2', Buffer.from([0, 255, 10, 13, 127])],
    ['snippets/ui/web/editor.js', Buffer.from('export const fixture = "é";')],
  ]);
  try {
    for (let generation = 0; generation < 2; generation++) {
      await mkdir(staging, { recursive: true });
      for (const [name, content] of assets) {
        const filename = path.join(staging, name);
        await mkdir(path.dirname(filename), { recursive: true });
        await writeFile(filename, content);
      }
      await execute(process.execPath, [generator, 'manifest'], {
        env: { ...process.env, TRUNK_STAGING_DIR: staging },
      });
      assert.deepEqual(await readdir(staging), ['admin']);
      // Same promotion order as Trunk, including replacement of an older build.
      if (generation > 0) {
        await rm(served, { recursive: true });
      }
      await rename(path.join(staging, 'admin'), served);
      const manifest = JSON.parse(await readFile(path.join(served, 'asset-manifest.json'), 'utf8'));
      assert.deepEqual(Object.keys(manifest).sort(), [...assets.keys()].sort());
      for (const [name, content] of assets) {
        assert.deepEqual(await readFile(path.join(served, name)), content);
        assert.equal(manifest[name], createHash('sha256').update(content).digest('hex'));
      }
    }
  } finally {
    // mkdtemp supplies this isolated absolute root; never remove a caller path.
    await rm(temporary, { recursive: true, force: true });
  }
});

test('a conflicting admin subtree fails packaging without overwriting its contents', async () => {
  const temporary = await mkdtemp(path.join(tmpdir(), 'ruphish-distribution-conflict-'));
  try {
    await mkdir(path.join(temporary, 'admin'));
    await writeFile(path.join(temporary, 'admin', 'sentinel.txt'), 'keep this file');
    await writeFile(path.join(temporary, 'index.html'), 'new document');
    await assert.rejects(
      execute(process.execPath, [generator, 'manifest'], {
        env: { ...process.env, TRUNK_STAGING_DIR: temporary },
      }),
    );
    assert.equal(
      await readFile(path.join(temporary, 'admin', 'sentinel.txt'), 'utf8'),
      'keep this file',
    );
    assert.equal(await readFile(path.join(temporary, 'index.html'), 'utf8'), 'new document');
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});
