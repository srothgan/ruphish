import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import { test } from 'node:test';
import { parseHTML } from 'linkedom';

const output = new URL('../dist/', import.meta.url);
const expected = new Map([
  ['index.html', 'Open-source phishing simulation & Gophish successor | Ruphish'],
  ['about/index.html', 'About | Ruphish'],
  ['blog/index.html', 'Blog | Ruphish'],
  ['blog/introducing-ruphish/index.html', 'Introducing Ruphish, an open-source successor to Gophish | Ruphish'],
  ['404.html', 'Page not found | Ruphish'],
]);

test('built public pages and docs use consistent document and social titles', async () => {
  const files = (await readdir(output, { recursive: true })).filter((path) => path.endsWith('.html'));
  assert.ok(files.some((path) => path.startsWith('docs/')), 'documentation pages were built');
  for (const path of expected.keys()) assert.ok(files.includes(path), path);
  for (const path of files) {
    const { document } = parseHTML(await readFile(new URL(path, output), 'utf8'));
    const titles = document.querySelectorAll('head title');
    assert.equal(titles.length, 1, path + ': exactly one document title');
    const title = titles[0].textContent;
    assert.match(title, /^.+ \| Ruphish$/, path + ': consistent branding suffix');
    assert.equal(title.split(' | Ruphish').length, 2, path + ': no repeated suffix');
    if (expected.has(path)) assert.equal(title, expected.get(path), path);
    assert.equal(document.querySelector('meta[property="og:title"]')?.getAttribute('content'), title, path + ': Open Graph title');
    const twitter = document.querySelector('meta[name="twitter:title"]');
    if (twitter) assert.equal(twitter.getAttribute('content'), title, path + ': Twitter title');
  }
});
