import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { JSDOM } from 'jsdom';
import { preparePage } from '../starter-templates.mjs';

const window = new JSDOM('').window;
globalThis.DOMParser = window.DOMParser;
const parse = (html) => new DOMParser().parseFromString(html, 'text/html');
const root = new URL('../../../ruphish-http/starter-templates/learning-pages/', import.meta.url);
const companyLogo =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aZAAAAABJRU5ErkJggg==';
const options = JSON.stringify({ selector: '.learning-logo', height: 48 });
const methods = [
  ['slam', ['Sender', 'Links', 'Attachments', 'Message']],
  ['stop-challenge-protect', ['Stop', 'Challenge', 'Protect']],
  ['recognize-resist-report-delete', ['Recognize', 'Resist and report', 'Delete']],
];

for (const [slug, steps] of methods) {
  test(`${slug}: complete static lesson and independent company-logo copies`, async () => {
    const source = await readFile(new URL(`${slug}/learning.html`, root), 'utf8');
    const document = parse(source);
    const defaultLogo = document.querySelector('.learning-logo img');
    assert.equal(defaultLogo.alt, 'Ruphish');
    assert.match(defaultLogo.src, /^data:image\/png;base64,/);
    const bytes = Buffer.from(defaultLogo.src.split(',')[1], 'base64');
    assert.equal(bytes.subarray(1, 4).toString(), 'PNG');
    assert.ok(bytes.readUInt32BE(16) > 100, 'bundled logo is artwork, not a test pixel');
    assert.equal(document.querySelectorAll('h1').length, 1);
    assert.deepEqual(
      [...document.querySelectorAll('h2')].slice(0, steps.length).map((h) => h.textContent),
      steps,
    );
    assert.match(document.body.textContent, /This was a phishing simulation/);
    assert.match(document.body.textContent, /IT or security/);
    assert.equal(
      document.querySelectorAll(
        'script,form,input,button,iframe,object,embed,link,[contenteditable]',
      ).length,
      0,
    );
    assert.doesNotMatch(source, /\{\{|__RUPHISH_LOGO__|@import|url\s*\(/i);
    for (const image of document.images) {
      assert.match(image.src, /^data:image\/png;base64,/);
    }
    for (const anchor of document.querySelectorAll('a[href^="#"]')) {
      assert.ok(
        document.getElementById(anchor.getAttribute('href').slice(1)),
        'in-page navigation resolves',
      );
    }
    assert.equal(preparePage(source, false, '', '', true), source);
    const branded = parse(preparePage(source, false, options, companyLogo, true));
    assert.equal(branded.querySelector('.learning-logo img').src, companyLogo);
    assert.equal(branded.querySelector('.learning-logo img').alt, 'Company logo');
    assert.equal(
      branded.body.textContent,
      document.body.textContent,
      'logo replacement keeps all lesson text',
    );
    assert.equal(
      branded.querySelector('style').textContent,
      document.querySelector('style').textContent,
      'logo replacement keeps this page’s layout',
    );
    assert.notEqual(defaultLogo.src, companyLogo, 'original remains Ruphish branded');
    assert.throws(() => preparePage(source, true, options, companyLogo, true), /starter_invalid/);
    for (const invalid of ['', 'https://example.test/logo.png', 'data:image/svg+xml;base64,AAAA']) {
      assert.throws(
        () => preparePage(source, false, options, invalid, true),
        /starter_logo_required/,
      );
    }
  });
}
