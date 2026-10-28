// Node-only source reader. The demo bundler replaces this module with its static export.
// Authored HTML remains authoritative in the existing starter collection; no copied dataset.
import { readFileSync } from 'node:fs';
import source from './data.json' with { type: 'json' };

const root = new URL('../../crates/ruphish-http/starter-templates/', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
export const starterContent = Object.fromEntries(
  [
    ['templates', 'emails', 'email.html'],
    ['pages', 'landing-pages', 'landing.html'],
    ['learning_pages', 'learning-pages', 'learning.html'],
  ].map(([family, folder, filename]) => [
    family,
    source[family].map((entry) => {
      if (!/^[a-z0-9-]+$/.test(entry.starter)) {
        throw new Error('Invalid demo starter source');
      }
      const directory = `${folder}/${entry.starter}`;
      return {
        html: read(`${directory}/${filename}`),
        ...(family === 'templates'
          ? {
              subject: read(`${directory}/subject.txt`).trimEnd(),
              text: read(`${directory}/email.txt`),
            }
          : {}),
      };
    }),
  ]),
);

// Embed the original licenses and provenance in the shipped demo JS banner.
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
