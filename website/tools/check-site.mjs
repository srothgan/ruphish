import { existsSync, readFileSync, statSync } from 'node:fs';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseHTML } from 'linkedom';
import { filesBelow } from './docs.mjs';

const dist = fileURLToPath(new URL('../dist/', import.meta.url));
const pages = filesBelow(dist).filter((file) => file.endsWith('.html'));
const documents = new Map(pages.map((file) => [file, parseHTML(readFileSync(file, 'utf8')).document]));
const failures = new Set();
for (const [file, document] of documents) {
  const url = `https://website.test/${relative(dist, file).replaceAll('\\', '/').replace(/index\.html$/, '')}`;
  for (const node of document.querySelectorAll('a[href], img[src], script[src], link[href]')) {
    const value = node.getAttribute('href') ?? node.getAttribute('src');
    if (!value || /^(mailto:|tel:|data:|javascript:)/i.test(value)) continue;
    const target = new URL(value, url);
    if (target.origin !== 'https://website.test') {
      if (node.localName === 'a' && ['http:', 'https:'].includes(target.protocol)) {
        if (node.getAttribute('target') !== '_blank' || !node.getAttribute('rel')?.split(/\s+/).includes('noopener')) {
          failures.add(`${relative(dist, file)}: external link must open a new tab with noopener: ${value}`);
        }
      }
      continue;
    }
    let path = resolve(dist, '.' + decodeURIComponent(target.pathname));
    if (existsSync(path) && statSync(path).isDirectory()) path = resolve(path, 'index.html');
    if (!existsSync(path)) failures.add(`${relative(dist, file)}: missing ${target.pathname}`);
    else if (target.hash && documents.has(path)) {
      const id = decodeURIComponent(target.hash.slice(1));
      if (!documents.get(path).getElementById(id)) failures.add(`${relative(dist, file)}: missing anchor ${target.pathname}${target.hash}`);
    }
  }
  if (!document.querySelector('title')?.textContent?.trim()) failures.add(`${relative(dist, file)}: missing title`);
  if (!document.querySelector('meta[name="description"]')?.getAttribute('content')) failures.add(`${relative(dist, file)}: missing description`);
  for (const image of document.querySelectorAll('img')) if (!image.hasAttribute('alt')) failures.add(`${relative(dist, file)}: image missing alt text`);
}
if (!existsSync(resolve(dist, 'pagefind/pagefind.js'))) failures.add('Missing Pagefind search index.');
if (failures.size) {
  console.error([...failures].sort().join('\n'));
  process.exitCode = 1;
} else console.log(`Verified ${pages.length} HTML pages: local links, external link targets, anchors, assets, page metadata, image alternatives, and search index.`);
