import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { packageDemo } from '../vercel-output.mjs';

async function fixture(t) {
    const root = await mkdtemp(join(tmpdir(), 'ruphish-vercel-'));
    t.after(() => rm(root, { recursive: true, force: true }));
    const source = join(root, 'source'), output = join(root, 'output');
    const files = {
        'index.html': '<html>Demo fixture</html>',
        'visual-frame.js': 'export const editor = true;',
        'visual-frame.css': 'body {}',
        'THIRD-PARTY-NOTICES.txt': 'Fixture notice',
        'ui.wasm': Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]),
        'snippets/demo.mjs': 'export const demo = true;',
    };
    const manifest = {};
    for (const [name, bytes] of Object.entries(files)) {
        await mkdir(dirname(join(source, name)), { recursive: true });
        await writeFile(join(source, name), bytes);
        manifest[name] = createHash('sha256').update(bytes).digest('hex');
    }
    await writeFile(join(source, 'asset-manifest.json'), JSON.stringify(manifest));
    return { root, source, output, files };
}

test('package preserves validated bytes and supplies explicit browser MIME types', async t => {
    const { source, output, files } = await fixture(t);
    await packageDemo(source, output);
    assert.deepEqual((await readdir(output)).sort(), ['config.json', 'static']);
    for (const [name, bytes] of Object.entries(files)) {
        assert.deepEqual(await readFile(join(output, 'static', name)), Buffer.from(bytes));
    }
    const config = JSON.parse(await readFile(join(output, 'config.json')));
    assert.equal(config.version, 3);
    assert.equal(config.overrides['ui.wasm'].contentType, 'application/wasm');
    assert.equal(config.overrides['snippets/demo.mjs'].contentType, 'text/javascript; charset=utf-8');

    // Exercise the emitted rules in order, including the documented filesystem phase.
    const route = (path, method = 'GET') => {
        for (const rule of config.routes) {
            if (rule.handle === 'filesystem') {
                if (Object.hasOwn(files, path.slice(1))) return path;
            } else if (new RegExp(`^(?:${rule.src})$`).test(path) && (!rule.methods || rule.methods.includes(method))) {
                if (!rule.continue) return rule.status ?? rule.dest;
            }
        }
    };
    for (const path of ['/', '/login', '/campaigns/3', '/groups/1/edit']) assert.equal(route(path), '/index.html');
    assert.equal(route('/campaigns/3', 'HEAD'), '/index.html');
    assert.equal(route('/ui.wasm'), '/ui.wasm');
    assert.equal(route('/snippets/demo.mjs'), '/snippets/demo.mjs');
    for (const path of ['/api', '/api/campaigns', '/auth/session', '/internal/status', '/missing.wasm', '/missing.mjs']) {
        assert.equal(route(path), 404, path);
    }
    assert.equal(route('/login', 'POST'), 405);
});

test('corrupted or unmanifested output cannot become a deployment artifact', async t => {
    const { source, output } = await fixture(t);
    await writeFile(join(source, 'visual-frame.js'), 'stale');
    await assert.rejects(packageDemo(source, output), /Asset integrity failed/);
    await writeFile(join(source, 'extra.js'), 'unexpected');
    await assert.rejects(packageDemo(source, output), /inconsistent asset manifest/);
});

test('hidden files and existing output are rejected instead of publishing stale or private data', async t => {
    const { source, output } = await fixture(t);
    await mkdir(output);
    await writeFile(join(output, 'stale.txt'), 'keep');
    await assert.rejects(packageDemo(source, output), /must be empty/);
    assert.equal(await readFile(join(output, 'stale.txt'), 'utf8'), 'keep');
    await writeFile(join(source, '.env'), 'synthetic fixture');
    await assert.rejects(packageDemo(source, output), /Unsupported asset path/);
});
