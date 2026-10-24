// Functional bridge: real catalog responses -> production UI transform -> ordinary API payloads.
import {readFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
import {JSDOM} from 'jsdom';
import {prepareLanding} from '../../starter-templates.mjs';
const dom = new JSDOM('');
globalThis.DOMParser = dom.window.DOMParser;
const entries = JSON.parse(await readFile(process.argv[2], 'utf8'));
const logo = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aZAAAAABJRU5ErkJggg==';
const copies = [];
for (const entry of entries.filter(entry => entry.summary.kind === 'landing_page')) {
  for (const branded of entry.summary.company_logo ? [false,true] : [false]) {
    const html = prepareLanding(entry.resource.html, true,
      branded ? JSON.stringify(entry.summary.company_logo) : '', branded ? logo : '');
    const document = new DOMParser().parseFromString(html, 'text/html');
    assert.equal(document.querySelector('input[type=email]').value,'{{.Email | html}}');
    assert.equal(document.querySelector('input[type=password]').value,'');
    if (branded) {
      const target = document.querySelector(entry.summary.company_logo.selector);
      assert.equal((target.tagName === 'IMG' ? target : target.querySelector('img')).src,logo);
    }
    copies.push({...entry.resource, html, name:`${entry.resource.name} - options ${branded}`});
  }
}
dom.window.close();
process.stdout.write(JSON.stringify(copies));
