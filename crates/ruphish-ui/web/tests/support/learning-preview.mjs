// Compare real served documents with the authored copies: static layout and content survive policy.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {JSDOM} from 'jsdom';

for (const {source, rendered} of JSON.parse(await readFile(process.argv[2], 'utf8'))) {
  const original = new JSDOM(source);
  const served = new JSDOM(rendered);
  const before = original.window.document;
  const after = served.window.document;
  assert.equal(after.title, before.title, 'the document title remains head metadata');
  const bodyText = after.body.textContent;
  if (before.title && !before.body.textContent.includes(before.title)) {
    assert.ok(!bodyText.includes(before.title), 'document title never leaks into visible content');
  }
  assert.equal(after.querySelectorAll('body title').length, 0);
  const headings = doc => [...doc.querySelectorAll('h1,h2,h3')].map(h => h.textContent);
  assert.deepEqual(headings(after), headings(before), 'all lesson sections survive sanitization');
  const layout = doc => [...doc.querySelectorAll('[class],[id]')].map(node => [node.tagName, node.className, node.id]);
  assert.deepEqual(layout(after), layout(before), 'layout containers and navigation targets survive sanitization');
  assert.equal(after.querySelector('style').textContent, before.querySelector('style').textContent);
  assert.equal(after.querySelector('.learning-logo img').src, before.querySelector('.learning-logo img').src);
  assert.equal(after.querySelector('.learning-logo img').alt, before.querySelector('.learning-logo img').alt);
  assert.equal(after.documentElement.lang, 'en');
  assert.equal(after.querySelectorAll('script,form,input,button,iframe,object,embed,link,[contenteditable]').length, 0);
  const text = doc => [...doc.querySelectorAll('p,li')].map(node => node.textContent.replace(/\s+/g, ' ').trim());
  assert.deepEqual(text(after), text(before), 'guidance, examples and reading order are preserved');
  for (const image of after.images) assert.match(image.src, /^data:image\/png;base64,/);
  for (const anchor of after.querySelectorAll('a[href^="#"]')) {
    assert.ok(after.getElementById(anchor.getAttribute('href').slice(1)), 'response-step links remain usable');
  }
  original.window.close();
  served.window.close();
}
