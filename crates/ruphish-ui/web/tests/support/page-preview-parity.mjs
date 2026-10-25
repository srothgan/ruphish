// Real API previews and public responses must differ only by the live landing collector.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {JSDOM} from 'jsdom';

for (const {kind, preview, served, title, visible} of JSON.parse(await readFile(process.argv[2], 'utf8'))) {
  const draft = new JSDOM(preview);
  const live = new JSDOM(served);
  const expected = draft.window.document;
  const actual = live.window.document;
  for (const doc of [expected, actual]) {
    assert.equal(doc.title, title);
    assert.equal(doc.querySelectorAll('head > title').length, 1);
    assert.ok(!doc.body.textContent.includes(title), 'title metadata does not become visible text');
    for (const text of visible) assert.ok(doc.body.textContent.includes(text), text);
    assert.equal(doc.querySelectorAll('iframe,object,[onclick],[onload],[onerror]').length, 0);
    assert.ok(!doc.documentElement.outerHTML.includes('outside.invalid'));
    assert.ok(!doc.documentElement.outerHTML.includes('unsafe()'));
  }
  assert.equal(expected.querySelectorAll('script').length, 0, 'previews have no collector');
  const collectors = actual.querySelectorAll('script');
  assert.equal(collectors.length, kind === 'landing' ? 1 : 0);
  for (const collector of collectors) {
    assert.ok(collector.nonce, 'live landing script is the nonce-bound collector');
    collector.remove();
  }
  assert.equal(actual.documentElement.outerHTML, expected.documentElement.outerHTML, 'preview and served content use the same preparation and sanitizer');
  const formCount = kind === 'landing' ? 1 : 0;
  assert.equal(expected.querySelectorAll('form').length, formCount);
  draft.window.close();
  live.window.close();
}
