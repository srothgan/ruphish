import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import type { Server } from 'node:http';
import { once } from 'node:events';
import test from 'node:test';
import { releaseResponse } from '../lib/release-selection.ts';
import { GET } from '../api/releases.ts';

// Independent asset expectations: do not build fixtures using the implementation's target list.
const platforms = [
  'x86_64-pc-windows-msvc',
  'aarch64-pc-windows-msvc',
  'x86_64-unknown-linux-gnu',
  'aarch64-unknown-linux-gnu',
  'aarch64-apple-darwin',
];
function release(version: string, overrides: Record<string, unknown> = {}) {
  const filenames = platforms.map(
    (target) => `ruphish-${version}-${target}.${target.includes('windows') ? 'zip' : 'tar.gz'}`,
  );
  filenames.push('SHA256SUMS', 'release-manifest.json', 'native-provenance.jsonl');
  return {
    tag_name: `v${version}`,
    draft: false,
    prerelease: version.includes('-'),
    published_at: '2026-10-29T10:00:00Z',
    assets: filenames.map((name) => ({
      name,
      state: 'uploaded',
      size: 123,
      browser_download_url: `https://github.com/srothgan/ruphish/releases/download/v${version}/${name}`,
    })),
    ...overrides,
  };
}
async function listen(server: Server) {
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const address = server.address();
  assert(address && typeof address !== 'string');
  return `http://127.0.0.1:${address.port}`;
}

test('release HTTP API selects by version, paginates, and fails visibly at upstream boundaries', async (t) => {
  let pages: unknown[][] = [[]];
  let upstreamStatus = 200;
  let malformed = false;
  const requested: number[] = [];
  const upstream = createServer((req, res) => {
    const url = new URL(req.url!, 'http://fixture');
    assert.equal(url.pathname, '/repos/srothgan/ruphish/releases');
    const page = Number(url.searchParams.get('page'));
    requested.push(page);
    res.writeHead(upstreamStatus, { 'Content-Type': 'application/json' });
    res.end(malformed ? '<html>bad gateway</html>' : JSON.stringify(pages[page - 1] ?? []));
  });
  const upstreamUrl = await listen(upstream);
  const fetcher: typeof fetch = (input, init) =>
    fetch(String(input).replace('https://api.github.com', upstreamUrl), init);
  const server = createServer(async (req, res) => {
    const response = await releaseResponse(
      new Request(`http://fixture${req.url}`, { method: req.method }),
      fetcher,
    );
    res.writeHead(response.status, Object.fromEntries(response.headers));
    res.end(await response.text());
  });
  const origin = await listen(server);
  t.after(() => {
    server.closeAllConnections();
    server.close();
    upstream.closeAllConnections();
    upstream.close();
  });

  await t.test(
    'default excludes previews; opt-in includes development and full releases',
    async () => {
      pages = [
        [
          release('0.1.0-dev.10'),
          release('0.1.0'),
          release('0.2.0-dev.2'),
          release('9.0.0', { draft: true }),
          release('8.0.0', { published_at: null }),
        ],
      ];
      let response = await fetch(`${origin}/api/releases?format=text`);
      assert.equal(response.status, 200);
      assert.equal(await response.text(), '0.1.0\n');
      assert.match(response.headers.get('cache-control')!, /s-maxage=300/);
      response = await fetch(`${origin}/api/releases?includePreviews=true`);
      const result = await response.json();
      assert.equal(response.status, 200);
      assert.equal(result.version, '0.2.0-dev.2');
      assert.equal(result.preview, true);
      assert.equal(result.assets.length, 5);
      assert.equal(
        result.assets[0].url,
        'https://github.com/srothgan/ruphish/releases/download/v0.2.0-dev.2/ruphish-0.2.0-dev.2-x86_64-pc-windows-msvc.zip',
      );
      pages[0].push(release('0.2.0'));
      response = await fetch(`${origin}/api/releases?includePreviews=true&format=text`);
      assert.equal(await response.text(), '0.2.0\n');
    },
  );
  await t.test(
    'numeric identifiers and pagination defeat API ordering and page-one assumptions',
    async () => {
      pages = [
        Array.from({ length: 100 }, () => release('0.1.0-dev.2')),
        [release('0.1.0-dev.10')],
      ];
      requested.length = 0;
      const response = await fetch(`${origin}/api/releases?includePreviews=true&format=text`);
      assert.equal(await response.text(), '0.1.0-dev.10\n');
      assert.deepEqual(requested, [1, 2]);
    },
  );
  await t.test(
    'no stable release is a 404 with opt-in guidance, never a preview fallback',
    async () => {
      pages = [[release('0.1.0-dev.1')]];
      const response = await fetch(`${origin}/api/releases?format=text`);
      assert.equal(response.status, 404);
      assert.match(await response.text(), /--include-previews/);
      assert.equal(response.headers.get('cache-control'), 'no-store');
    },
  );
  await t.test(
    'an incomplete latest release does not silently select an older release',
    async () => {
      const incomplete = release('0.2.0');
      incomplete.assets.pop();
      pages = [[incomplete, release('0.1.0')]];
      const response = await fetch(`${origin}/api/releases`);
      assert.equal(response.status, 503);
      assert.match((await response.json()).error, /temporarily unavailable/);
    },
  );
  await t.test(
    'wrong classification, asset origins, missing metadata, and duplicate assets fail closed',
    async () => {
      const wrongOrigin = release('0.1.0');
      wrongOrigin.assets[0].browser_download_url = 'https://example.invalid/evil.zip';
      const duplicate = release('0.1.0');
      duplicate.assets.push(duplicate.assets[0]);
      for (const bad of [
        release('0.1.0', { prerelease: true }),
        wrongOrigin,
        duplicate,
        { tag_name: 'v0.1.0' },
      ]) {
        pages = [[bad]];
        assert.equal((await fetch(`${origin}/api/releases`)).status, 503);
      }
    },
  );
  await t.test(
    'network HTTP failures and malformed JSON never become successful selection',
    async () => {
      pages = [[release('0.1.0')]];
      upstreamStatus = 429;
      assert.equal((await fetch(`${origin}/api/releases`)).status, 503);
      upstreamStatus = 200;
      malformed = true;
      assert.equal((await fetch(`${origin}/api/releases`)).status, 503);
      malformed = false;
    },
  );
  await t.test('bad queries are rejected before contacting GitHub', async () => {
    requested.length = 0;
    for (const query of [
      'includePreviews=yes',
      'includePreviews=true&includePreviews=false',
      'format=xml',
      'repository=other/repo',
    ]) {
      assert.equal((await fetch(`${origin}/api/releases?${query}`)).status, 400);
    }
    assert.deepEqual(requested, []);
  });
  await t.test('the actual Vercel entry point returns a Web Response', async () => {
    // Invalid input needs no upstream seam and exercises the deployed export itself.
    const response = await GET(new Request('https://ruphish.com/api/releases?format=invalid'));
    assert.equal(response.status, 400);
  });
});
