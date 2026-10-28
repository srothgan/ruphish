// Node build adapter for the same manifest and authored files embedded by the native server.
// Browser builds substitute these static exports; no filesystem APIs reach the browser.
import { readFileSync } from 'node:fs';
import manifest from '../../starter-templates/catalog.json' with { type: 'json' };

const root = new URL('../../starter-templates/', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
export const starterCatalog = manifest.map((entry) => {
  let html = read(entry.html);
  if (entry.script) {
    html = html.replace(
      '</body>',
      () => `<script data-starter-interaction>${read(entry.script)}</script></body>`,
    );
  }
  return {
    summary: {
      id: entry.id,
      name: entry.name,
      brand: entry.brand,
      kind: entry.kind,
      prefill: entry.kind === 'landing_page',
      company_logo: entry.company_logo ?? null,
    },
    resource: {
      name: entry.name,
      html,
      ...(entry.kind === 'email'
        ? { subject: read(entry.subject).trimEnd(), text: read(entry.text), attachments: [] }
        : entry.kind === 'landing_page'
          ? { redirect_url: '' }
          : {}),
    },
  };
});

export const starterNotices = [
  'licenses/THESVG-LICENSE',
  'licenses/SIMPLE-ICONS-LICENSE.md',
  'licenses/SIMPLE-ICONS-DISCLAIMER.md',
  'licenses/sources.json',
  'licenses/additional-sources.json',
  'learning-pages/README.md',
]
  .map((path) => `${path}\n${read(path)}`)
  .join('\n\n');
