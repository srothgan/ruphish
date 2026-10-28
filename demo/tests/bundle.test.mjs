import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

test('the generated browser bundle logs in and reads real starter content without network or Node imports', async (t) => {
  const filename = new URL('../../crates/ruphish-ui/web/generated/demo.js', import.meta.url);
  const source = await readFile(filename, 'utf8');
  assert.ok(!/from\s*["']node:|import\(["']node:/.test(source));
  assert.match(source, /Copyright \(c\) 2025 thesvg\.org/);
  t.mock.method(globalThis, 'fetch', () => {
    throw new Error('No network fallback is allowed');
  });
  const memory = new Map();
  const previousWindow = globalThis.window;
  globalThis.window = {
    sessionStorage: {
      getItem: (key) => memory.get(key),
      setItem: (key, value) => memory.set(key, value),
      removeItem: (key) => memory.delete(key),
    },
  };
  try {
    const bundle = await import(filename.href);
    const read = async (path, body) => {
      const request = new Request(
        `https://demo.example.test${path}`,
        body ? { method: 'POST', body: new URLSearchParams(body) } : {},
      );
      const response = await bundle.requestPromise(
        bundle.startRequest(request, 1000, () => {}, false),
      );
      assert.equal(response.status, 200, path);
      return JSON.parse(response.body);
    };
    await read('/auth/login', { username: 'demo', password: 'demodemo' });
    assert.equal((await read('/auth/session')).user.username, 'demo');
    const catalog = await read('/api/starter-templates/');
    assert.equal(catalog.filter((row) => row.kind === 'email').length, 23);
    assert.equal(catalog.filter((row) => row.kind === 'landing_page').length, 25);
    assert.equal(catalog.filter((row) => row.kind === 'learning_page').length, 3);
    const interactive = await read('/api/starter-templates/page-microsoft-two-step');
    assert.match(interactive.resource.html, /<script data-starter-interaction>/);
    assert.match(interactive.resource.html, /window\.ruphish\.submit\(form\)/);
    const invitation = await read('/api/starter-templates/email-onedrive-document-invitation');
    assert.match(invitation.resource.subject, /Project handover\.docx/);
    assert.match((await read('/api/templates/1')).html, /Microsoft/);
    assert.match((await read('/api/pages/1')).html, /Sign in to your account/);
    assert.match((await read('/api/learning-pages/1')).preview_html, /SLAM/);
    const directory = await read('/api/campaigns/directory?filter=executions&after=');
    assert.equal(directory.items.length, 4);
    const completed = directory.items.find((row) => row.status === 'Completed');
    assert.ok(completed);
    assert.ok((await read(`/api/campaigns/${completed.id}/results`)).event_total > 0);
    const sample = await readFile(
      new URL('../../crates/ruphish-ui/assets/recipient-template.csv', import.meta.url),
      'utf8',
    );
    const review = async (content) => {
      const request = new Request('https://demo.example.test/api/import/group/review', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ content }),
      });
      return bundle.requestPromise(bundle.startRequest(request, 1000, () => {}, false));
    };
    const imported = await review(sample);
    assert.equal(imported.status, 200);
    assert.deepEqual(
      JSON.parse(imported.body).targets.map((r) => r.email),
      ['alex@example.test', 'sam@example.test'],
    );
    assert.equal((await review('Wrong,Headers\na,b\n')).status, 400);
    const request = new Request(
      `https://demo.example.test/api/campaigns/${completed.id}/export?format=csv&scope=recipients&limit=1`,
    );
    const download = await bundle.requestPromise(
      bundle.startRequest(request, 1000, () => {}, true),
    );
    assert.equal(download.status, 200);
    assert.ok(download.file instanceof Blob);
    const exported = JSON.parse((await review(await download.file.text())).body);
    assert.ok(exported.columns.includes('Send Date (UTC)'));
    assert.ok(exported.columns.includes('Modified Date (UTC)'));
    assert.equal(exported.targets.length, completed.total);
    const malformed = new Request(
      `https://demo.example.test/api/campaigns/${completed.id}/export?format=xlsx&from=invalid`,
    );
    const rejected = await bundle.requestPromise(
      bundle.startRequest(malformed, 1000, () => {}, true),
    );
    assert.equal(rejected.status, 400);
    assert.equal(rejected.file, null);
    assert.ok([...memory.values()].every((value) => value !== 'demodemo'));
  } finally {
    if (previousWindow === undefined) {
      delete globalThis.window;
    } else {
      globalThis.window = previousWindow;
    }
  }
});
