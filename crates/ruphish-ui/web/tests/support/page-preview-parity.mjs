// Real previews and public responses share content, with explicit landing endpoint/base differences.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { JSDOM } from 'jsdom';

for (const { kind, preview, served, title, visible, endpoint } of JSON.parse(
  await readFile(process.argv[2], 'utf8'),
)) {
  const draft = new JSDOM(preview);
  const live = new JSDOM(served);
  const expected = draft.window.document;
  const actual = live.window.document;
  for (const doc of [expected, actual]) {
    assert.equal(doc.title, title);
    assert.equal(doc.querySelectorAll('head > title').length, 1);
    assert.ok(!doc.body.textContent.includes(title), 'title metadata does not become visible text');
    for (const text of visible) {
      assert.ok(doc.body.textContent.includes(text), text);
    }
    assert.equal(doc.querySelectorAll('iframe,object').length, 0);
    if (kind === 'learning') {
      assert.equal(doc.querySelectorAll('script,[onclick],[onload],[onerror]').length, 0);
      assert.ok(!doc.documentElement.outerHTML.includes('outside.invalid'));
      assert.ok(!doc.documentElement.outerHTML.includes('unsafe()'));
    } else {
      assert.deepEqual(
        [...doc.querySelectorAll('script:not([data-ruphish-collector])')].map((s) => s.textContent),
        ['unsafe()'],
        'authored scripts are preserved without executing them in this DOM comparison',
      );
      assert.equal(doc.querySelector('img').getAttribute('src'), 'https://outside.invalid/pixel');
      assert.equal(doc.querySelector('img').getAttribute('onerror'), 'unsafe()');
      assert.equal(doc.querySelector('form').getAttribute('action'), null);
      assert.equal(doc.querySelector('form').getAttribute('target'), null);
      const policy = doc.querySelector('meta[http-equiv="Content-Security-Policy"]').content;
      for (const directive of ["form-action 'none'", "frame-src 'none'", "object-src 'none'"]) {
        assert.ok(policy.includes(directive), directive);
      }
      const collectors = doc.querySelectorAll('script[data-ruphish-collector]');
      assert.equal(collectors.length, 1);
      const collector = collectors[0];
      assert.equal(doc.querySelector('script'), collector, 'managed setup precedes authored code');
      assert.ok(
        collector.textContent.includes(
          `const endpoint=${JSON.stringify(doc === expected ? null : endpoint)};`,
        ),
        'preview cannot submit a live event; the live collector uses the exact recipient endpoint',
      );
      const bases = doc.querySelectorAll('base');
      assert.equal(bases.length, 1);
      assert.equal(bases[0].href, doc === expected ? 'https://preview.invalid/' : endpoint);
      collector.remove();
      bases[0].remove();
    }
  }
  assert.equal(
    actual.documentElement.outerHTML,
    expected.documentElement.outerHTML,
    'preview and served content use the same preparation after verified runtime differences',
  );
  const formCount = kind === 'landing' ? 1 : 0;
  assert.equal(expected.querySelectorAll('form').length, formCount);
  draft.window.close();
  live.window.close();
}
