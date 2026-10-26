import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

test('the generated browser bundle logs in and reads real starter content without network or Node imports', async t => {
    const filename = new URL('../../crates/ruphish-ui/web/generated/demo.js', import.meta.url);
    const source = await readFile(filename, 'utf8');
    assert.ok(!/from\s*["']node:|import\(["']node:/.test(source));
    assert.match(source, /Copyright \(c\) 2025 thesvg\.org/);
    t.mock.method(globalThis, 'fetch', () => { throw new Error('No network fallback is allowed'); });
    const memory = new Map(), previousWindow = globalThis.window;
    globalThis.window = { sessionStorage: { getItem: key => memory.get(key), setItem: (key, value) => memory.set(key, value), removeItem: key => memory.delete(key) } };
    try {
        const bundle = await import(filename.href);
        const read = async (path, body) => {
            const request = new Request(`https://demo.example.test${path}`, body ? { method: 'POST', body: new URLSearchParams(body) } : {});
            const response = await bundle.requestPromise(bundle.startRequest(request, 1000, () => {}, false));
            assert.equal(response.status, 200, path);
            return JSON.parse(response.body);
        };
        await read('/login', { username: 'demo', password: 'demodemo' });
        assert.equal((await read('/auth/session')).user.username, 'demo');
        assert.match((await read('/api/templates/1')).html, /Microsoft/);
        assert.match((await read('/api/pages/1')).html, /Sign in to your account/);
        assert.match((await read('/api/learning-pages/1')).preview_html, /SLAM/);
        assert.ok([...memory.values()].every(value => value !== 'demodemo'));
    } finally {
        if (previousWindow === undefined) delete globalThis.window;
        else globalThis.window = previousWindow;
    }
});
