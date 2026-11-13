import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import { parseHTML } from 'linkedom';

const output = new URL('../dist/', import.meta.url);
const pages = [
  'index.html',
  'about/index.html',
  'blog/index.html',
  'blog/introducing-ruphish/index.html',
  '404.html',
  'docs/index.html',
  'docs/getting-started/install-ruphish/index.html',
];

for (const path of pages) {
  test(`${path} has shared theming and keeps the documentation reset within docs`, async () => {
    const { document } = parseHTML(await readFile(new URL(path, output), 'utf8'));
    const controllers = [...document.querySelectorAll('script')].filter((script) =>
      script.textContent.includes("const storageKey = 'starlight-theme'"),
    );
    assert.equal(controllers.length, 1, 'only one preference controller owns the page');
    assert.ok(document.head.contains(controllers[0]), 'initialization happens before the body');
    assert.ok(document.querySelector('select[aria-label="Color theme"]'));
    assert.equal(document.querySelector('starlight-theme-select'), null);
    if (!path.startsWith('docs/')) {
      assert.ok(document.body.classList.contains('site-page'));
      for (const link of document.querySelectorAll('link[rel="stylesheet"]')) {
        const href = link.getAttribute('href');
        const css = await readFile(new URL(href.replace(/^\//, ''), output), 'utf8');
        assert.doesNotMatch(
          css,
          /background-color:\s*var\(--sl-color-bg\)/,
          'public pages must not pull in the Starlight body reset',
        );
      }
    }
    if (path === 'index.html') {
      assert.equal(document.querySelector('starlight-tabs'), null);
      const blocks = [...document.querySelectorAll('.command .astro-code')];
      assert.equal(blocks.length, 3);
      for (const block of blocks) {
        assert.match(block.outerHTML, /--shiki-light/);
        assert.match(block.outerHTML, /--shiki-dark/);
      }
      for (const tab of document.querySelectorAll('[data-tab-group] [role="tab"]')) {
        const panel = document.getElementById(tab.getAttribute('aria-controls'));
        assert.ok(panel);
        assert.equal(panel.getAttribute('aria-labelledby'), tab.id);
        assert.equal(panel.hidden, tab.getAttribute('aria-selected') !== 'true');
      }
    }
  });
}
