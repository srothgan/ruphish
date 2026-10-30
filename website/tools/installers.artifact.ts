import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

test('the Astro build emits the exact canonical installer bytes at both advertised paths', () => {
  for (const extension of ['sh', 'ps1']) {
    const source = readFileSync(new URL(`../public/install.${extension}`, import.meta.url));
    const emitted = readFileSync(new URL(`../dist/install.${extension}`, import.meta.url));
    assert.deepEqual(emitted, source);
    assert(!emitted.includes(Buffer.from('\r\n')), 'Piped installers must retain LF source lines');
    assert(
      !emitted.toString().startsWith('<!DOCTYPE'),
      'Installer URL must not be an HTML fallback',
    );
  }
});
