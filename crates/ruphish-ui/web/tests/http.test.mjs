import test from 'node:test';
import assert from 'node:assert/strict';
import { startRequest } from '../http.mjs';

function clock(t) {
  let fire;
  let cleared = 0;
  t.mock.method(globalThis, 'setTimeout', (callback) => {
    fire = callback;
    return 17;
  });
  t.mock.method(globalThis, 'clearTimeout', () => {
    cleared++;
  });
  return { expire: () => fire(), cleared: () => cleared };
}
test('complete response retains status/revision and releases its deadline', async (t) => {
  const timer = clock(t);
  t.mock.method(
    globalThis,
    'fetch',
    async () => new Response('{"id":7}', { status: 201, headers: { etag: '"r7"' } }),
  );
  assert.deepEqual(await startRequest(new Request('https://admin.test/'), 15000).promise, {
    status: 201,
    etag: '"r7"',
    body: '{"id":7}',
  });
  assert.equal(timer.cleared(), 1);
});
test('stalled headers time out, abort the request and permit a fresh attempt', async (t) => {
  const timer = clock(t);
  let signal;
  t.mock.method(globalThis, 'fetch', (_request, options) => {
    signal = options.signal;
    return new Promise((_resolve, reject) =>
      signal.addEventListener('abort', () => reject(new Error('secret URL and token')), {
        once: true,
      }),
    );
  });
  const pending = startRequest(new Request('https://admin.test/'), 15000);
  const rejected = assert.rejects(pending.promise, /^Error: request_timeout$/);
  timer.expire();
  await rejected;
  assert(signal.aborted);
  t.mock.method(globalThis, 'fetch', async () => new Response('{}'));
  assert.equal((await startRequest(new Request('https://admin.test/'), 15000).promise).status, 200);
});
test('deadline covers a body that stalls after successful headers', async (t) => {
  const timer = clock(t);
  let bodyStarted;
  const started = new Promise((resolve) => {
    bodyStarted = resolve;
  });
  t.mock.method(globalThis, 'fetch', async (_request, { signal }) => ({
    status: 200,
    headers: new Headers(),
    text: () =>
      new Promise((_resolve, reject) => {
        bodyStarted();
        signal.addEventListener('abort', () => reject(new Error('private body')), { once: true });
      }),
  }));
  const pending = startRequest(new Request('https://admin.test/'), 15000);
  const rejected = assert.rejects(pending.promise, /^Error: request_timeout$/);
  await started;
  timer.expire();
  await rejected;
});
test('navigation cancellation aborts transport and suppresses a late success', async (t) => {
  const timer = clock(t);
  let finish;
  t.mock.method(
    globalThis,
    'fetch',
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  let headers = 0;
  const pending = startRequest(new Request('https://admin.test/'), 15000, () => {
    headers++;
  });
  const rejected = assert.rejects(pending.promise, /^Error: request_unavailable$/);
  pending.cancel();
  finish(new Response('{"id":99}'));
  await rejected;
  assert.equal(headers, 0);
  assert(timer.cleared() > 0);
});

test('authentication loss is reported at headers even if the response body stalls', async (t) => {
  const timer = clock(t);
  let received;
  const headers = new Promise((resolve) => {
    received = resolve;
  });
  t.mock.method(globalThis, 'fetch', async (_request, { signal }) => ({
    status: 401,
    headers: new Headers(),
    text: () =>
      new Promise((_resolve, reject) => {
        signal.addEventListener('abort', () => reject(new Error('private diagnostic')), {
          once: true,
        });
      }),
  }));
  const pending = startRequest(new Request('https://admin.test/'), 15000, received);
  const rejected = assert.rejects(pending.promise, /^Error: request_timeout$/);
  assert.equal(await headers, 401);
  timer.expire();
  await rejected;
});

test('binary exports preserve bytes and errors remain readable JSON', async (t) => {
  clock(t);
  const bytes = new Uint8Array([0x50, 0x4b, 0, 255, 128]);
  t.mock.method(globalThis, 'fetch', async () => new Response(bytes));
  const result = await startRequest(new Request('https://admin.test/export'), 60000, () => {}, true)
    .promise;
  assert.deepEqual(new Uint8Array(await result.file.arrayBuffer()), bytes);
  assert.equal(result.body, '');
  t.mock.method(
    globalThis,
    'fetch',
    async () => new Response('{"message":"export_too_large"}', { status: 400 }),
  );
  const rejected = await startRequest(
    new Request('https://admin.test/export'),
    60000,
    () => {},
    true,
  ).promise;
  assert.equal(rejected.file, null);
  assert.equal(JSON.parse(rejected.body).message, 'export_too_large');
});
