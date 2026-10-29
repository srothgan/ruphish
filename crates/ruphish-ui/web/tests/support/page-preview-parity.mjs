// Preview and public landing content differ only by the collector endpoint and base URL.
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
    assert.equal(doc.querySelectorAll('iframe,frame,frameset,object,embed').length, 0);
    if (kind === 'learning') {
      assert.equal(
        doc.querySelectorAll('script,form,input,button,[onclick],[onload],[onerror]').length,
        0,
      );
      assert.ok(!doc.documentElement.outerHTML.includes('outside.invalid'));
      assert.ok(!doc.documentElement.outerHTML.includes('unsafe()'));
    } else {
      assert.equal(doc.querySelectorAll('script').length, 2);
      assert.equal(doc.querySelectorAll('script[data-ruphish-collector]').length, 1);
      assert.equal(doc.head.querySelector('script').hasAttribute('data-ruphish-collector'), true);
      assert.equal(
        doc.querySelector('script:not([data-ruphish-collector])').textContent,
        'unsafe()',
      );
      assert.equal(doc.querySelector('img').getAttribute('src'), 'https://outside.invalid/pixel');
      assert.equal(doc.querySelector('img').getAttribute('onerror'), 'unsafe()');
      assert.equal(
        doc.querySelectorAll('form[action],form[target],[formaction],[formtarget]').length,
        0,
      );
      const policy = doc.querySelector('meta[http-equiv="Content-Security-Policy"]').content;
      for (const directive of ["form-action 'none'", "frame-src 'none'", "object-src 'none'"]) {
        assert.ok(policy.includes(directive), directive);
      }
      assert.equal(doc.querySelectorAll('base').length, 1);
      assert.equal(
        doc.querySelector('script'),
        doc.querySelector('[data-ruphish-collector]'),
        'managed setup precedes authored code',
      );
    }
  }
  if (kind === 'landing') {
    const previewBase = expected.querySelector('base');
    const liveBase = actual.querySelector('base');
    assert.equal(previewBase.href, 'https://preview.invalid/');
    assert.equal(liveBase.href, endpoint, 'live base uses the exact recipient endpoint');
    const previewCollector = expected.querySelector('[data-ruphish-collector]');
    const liveCollector = actual.querySelector('[data-ruphish-collector]');
    const liveEndpoint = `const endpoint=${JSON.stringify(endpoint)};`;
    assert.ok(liveCollector.textContent.includes(liveEndpoint));
    assert.ok(previewCollector.textContent.includes('const endpoint=null;'));
    // Normalize only these verified differences; compare all remaining markup and script content.
    liveCollector.textContent = liveCollector.textContent.replace(
      liveEndpoint,
      'const endpoint=null;',
    );
    liveBase.setAttribute('href', previewBase.getAttribute('href'));
  }
  assert.equal(
    actual.documentElement.outerHTML,
    expected.documentElement.outerHTML,
    'preview and served content use the same preparation for their page kind',
  );
  const formCount = kind === 'landing' ? 1 : 0;
  assert.equal(expected.querySelectorAll('form').length, formCount);
  draft.window.close();
  live.window.close();
}
