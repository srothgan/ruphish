import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { once } from 'node:events';
import { previewServer } from '../serve.mjs';

test('static preview supports deep links and WASM without turning API or missing assets into HTML', async () => {
    const directory = await mkdtemp(join(tmpdir(), 'ruphish-demo-preview-'));
    const server = previewServer(directory);
    try {
        await writeFile(join(directory, 'index.html'), '<html>demo</html>');
        await writeFile(join(directory, 'app.wasm'), new Uint8Array([0, 97, 115, 109]));
        server.listen(0, '127.0.0.1'); await once(server, 'listening');
        const base = `http://127.0.0.1:${server.address().port}`;
        for (const path of ['/', '/login', '/groups/1', '/campaigns/drafts/00000000-0000-4000-8000-000000000001']) {
            const response = await fetch(base + path);
            assert.equal(response.status, 200);
            assert.equal(await response.text(), '<html>demo</html>');
            assert.equal(response.headers.get('x-robots-tag'), 'noindex, nofollow');
        }
        const wasm = await fetch(base + '/app.wasm');
        assert.equal(wasm.headers.get('content-type'), 'application/wasm');
        assert.deepEqual(new Uint8Array(await wasm.arrayBuffer()), new Uint8Array([0, 97, 115, 109]));
        for (const path of ['/api/groups', '/auth/session', '/internal/password', '/missing.js', '/missing.wasm', '/%2e%2e%5coutside']) assert.equal((await fetch(base + path)).status, 404);
        assert.equal((await fetch(base + '/login', { method: 'POST' })).status, 405);
    } finally {
        server.closeAllConnections(); await new Promise(resolve => server.close(resolve));
        await rm(directory, { recursive: true, force: true });
    }
});
