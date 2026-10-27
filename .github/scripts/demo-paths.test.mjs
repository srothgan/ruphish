import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { matchesGlob } from 'node:path';

// Read the actual, flat demo group; CI's paths-filter owns change detection.
const yaml = readFileSync(new URL('../paths.yml', import.meta.url), 'utf8');
const section = yaml.match(/^demo:\r?\n((?:[^\S\r\n].*\r?\n|\r?\n|#.*\r?\n)*)/m)?.[1];
assert.ok(section, 'Missing demo path group');
const patterns = [...section.matchAll(/^  - '([^']+)'/gm)].map(match => match[1]);
const selected = path => patterns.some(pattern => matchesGlob(path, pattern));

test('demo selection includes browser dependencies, authored content and build ownership', () => {
    for (const path of [
        'crates/ruphish-ui/src/account.rs', 'crates/ruphish-ui/web/package-lock.json',
        'crates/ruphish-domain/src/lib.rs', 'crates/ruphish-contracts/src/lib.rs',
        'crates/ruphish-http/starter-templates/emails/example/email.html',
        'assets/website-demo/data.json', 'demo/vercel-output.mjs',
        'Cargo.lock', 'Cargo.toml', 'Trunk.toml', 'rust-toolchain.toml',
        '.cargo/config.toml', '.github/actions/trunk/linux-x64.sha256',
        '.github/workflows/pr.yml', '.github/workflows/demo.yml', '.github/paths.yml',
    ]) assert.equal(selected(path), true, path);
});

test('backend implementation, backend tests and website prose do not rebuild the demo', () => {
    for (const path of [
        'crates/ruphish-adapters/src/sqlite.rs', 'crates/ruphish-application/src/lib.rs',
        'crates/ruphish-http/src/server.rs', 'crates/ruphish/src/main.rs',
        'tests/startup_process.rs', 'docs/contributing/demo.md',
        'website/src/pages/index.astro', 'website/package-lock.json', 'README.md',
    ]) assert.equal(selected(path), false, path);
});
