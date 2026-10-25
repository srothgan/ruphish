// Functional bridge: real catalog responses -> production UI transform -> ordinary API payloads.
import {readFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
import {JSDOM} from 'jsdom';
import {preparePage} from '../../starter-templates.mjs';
const dom = new JSDOM('');
globalThis.DOMParser = dom.window.DOMParser;
const entries = JSON.parse(await readFile(process.argv[2], 'utf8'));
const logo = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aZAAAAABJRU5ErkJggg==';
const copies = [];
for (const entry of entries.filter(entry => entry.summary.kind !== 'email')) {
  const learning = entry.summary.kind === 'learning_page';
  for (const branded of entry.summary.company_logo ? [false,true] : [false]) {
    const html = preparePage(entry.resource.html, !learning,
      branded ? JSON.stringify(entry.summary.company_logo) : '', branded ? logo : '', learning);
    const document = new DOMParser().parseFromString(html, 'text/html');
    if (!learning) {
      assert.equal(document.querySelector('input[type=email]').value,'{{.Email | html}}');
      assert.equal(document.querySelector('input[type=password]').value,'');
    }
    if (branded) {
      const target = document.querySelector(entry.summary.company_logo.selector);
      assert.equal((target.tagName === 'IMG' ? target : target.querySelector('img')).src,logo);
    }
    copies.push({kind:entry.summary.kind, source_id:entry.summary.id, branded,
      resource:{...entry.resource, html, name:`${entry.resource.name} - options ${branded}`}});
  }
}
dom.window.close();
process.stdout.write(JSON.stringify(copies));
