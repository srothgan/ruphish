import { test } from 'node:test';
import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { docRoute, isPublished, resolveDocLink, sourceRoot } from './docs.mjs';

test('Markdown routes preserve directories and collapse index pages', () => {
  assert.equal(docRoute('index.md'), '/docs/');
  assert.equal(docRoute('getting-started/index.md'), '/docs/getting-started/');
  assert.equal(docRoute('user-guide/results/metrics.md'), '/docs/user-guide/results/metrics/');
});
test('archives and private content never enter the publication set', () => {
  for (const path of ['old/startup.md', '.private/secret.md', '_draft.md', 'api/_draft/index.md']) assert.equal(isPublished(path), false);
  assert.equal(isPublished('getting-started/installation.md'), true);
});
test('relative links retain queries and anchors and point at real published pages', () => {
  const source = resolve(sourceRoot, 'getting-started/index.md');
  assert.equal(resolveDocLink('installation.md#build', source), '/docs/getting-started/installation/#build');
  assert.equal(resolveDocLink('../index.md?view=full#start', source), '/docs/?view=full#start');
  assert.equal(resolveDocLink('../../README.md', source), 'https://github.com/srothgan/ruphish/blob/main/README.md');
  for (const url of ['#build', 'https://example.test/', 'mailto:author@example.test', '/about/']) assert.equal(resolveDocLink(url, source), url);
  assert.throws(() => resolveDocLink('missing.md', source), /Missing/);
  assert.throws(() => resolveDocLink('../old/startup.md', source), /unpublished/);
  assert.throws(() => resolveDocLink('../../../unpublished.md', source), /Missing/);
});
