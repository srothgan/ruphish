import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { until, fetchJson, deliveryAccepted, deliverySummary } from './wait.mjs';

async function server(t, handle) {
  const service = createServer(handle);
  service.listen(0, '127.0.0.1');
  await once(service, 'listening');
  t.after(async () => {
    const closed = new Promise((resolve) => service.close(resolve));
    service.closeAllConnections();
    await closed;
  });
  return `http://127.0.0.1:${service.address().port}/delivery`;
}

function delivery(state, overrides = {}) {
  return {
    campaign: { id: 1, status: 'In progress', domain_block: null },
    mailer_enabled: true,
    counts: { [state]: 1 },
    items: [
      {
        id: 1,
        email: 'private-recipient@example.test',
        state,
        code: state === 'blocked' ? 'smtp_tls_failed' : '',
        attempt_count: 1,
        attempts: [{ state, stage: 'tls', smtp_code: 454, message_id: 'private-message-id' }],
      },
    ],
    ...overrides,
  };
}

function wait(url, options = {}) {
  return until((signal) => fetchJson(url, { signal }), deliveryAccepted, 'SMTP acceptance', {
    intervalMs: 5,
    timeoutMs: 1_000,
    describe: deliverySummary,
    log: () => {},
    ...options,
  });
}

test(
  'delivery polling observes ready, retry and accepted responses and reports progress',
  { timeout: 5_000 },
  async (t) => {
    const states = ['ready', 'retry', 'accepted'];
    let calls = 0;
    const url = await server(t, (_req, res) => res.end(JSON.stringify(delivery(states[calls++]))));
    const logs = [];
    const result = await wait(url, { log: (line) => logs.push(line) });
    assert.equal(result.counts.accepted, 1);
    assert.equal(calls, 3);
    assert(logs.some((line) => line.includes('"ready":1')));
    assert.equal(logs.at(-1), 'Completed SMTP acceptance');
  },
);

for (const state of ['blocked', 'failed', 'unknown', 'cancelled']) {
  test(
    `delivery polling fails immediately on ${state} with safe protocol diagnostics`,
    { timeout: 5_000 },
    async (t) => {
      let calls = 0;
      const url = await server(t, (_req, res) => {
        calls++;
        res.end(JSON.stringify(delivery(state)));
      });
      await assert.rejects(wait(url), (error) => {
        assert(error.message.includes(`delivery entered ${state}`));
        assert(error.message.includes('"stage":"tls"'));
        assert(error.message.includes('"smtp_code":454'));
        if (state === 'blocked') {
          assert(error.message.includes('smtp_tls_failed'));
        }
        assert(!error.message.includes('private-recipient'));
        assert(!error.message.includes('private-message-id'));
        return true;
      });
      assert.equal(calls, 1);
    },
  );
}

for (const [reason, overrides] of [
  ['delivery workers are disabled', { mailer_enabled: false }],
  [
    'domain blocked: domain_expired',
    { campaign: { id: 1, status: 'In progress', domain_block: 'domain_expired' } },
  ],
  [
    'campaign completed without SMTP acceptance',
    { campaign: { id: 1, status: 'Completed', domain_block: null } },
  ],
]) {
  test(`queued delivery fails when ${reason}`, { timeout: 5_000 }, async (t) => {
    const url = await server(t, (_req, res) =>
      res.end(JSON.stringify(delivery('ready', overrides))),
    );
    await assert.rejects(wait(url), (error) => error.message.includes(reason));
  });
}

test(
  'queued delivery expires with its last observation instead of waiting forever',
  { timeout: 5_000 },
  async (t) => {
    const url = await server(t, (_req, res) => res.end(JSON.stringify(delivery('ready'))));
    await assert.rejects(wait(url, { timeoutMs: 250 }), /deadline exceeded.*"ready":1/);
  },
);

for (const partialBody of [false, true]) {
  test(
    `deadline aborts HTTP ${partialBody ? 'body' : 'headers'} that never completes`,
    { timeout: 5_000 },
    async (t) => {
      let closeResponse;
      const closed = new Promise((resolve) => {
        closeResponse = resolve;
      });
      const url = await server(t, (_req, res) => {
        res.on('close', closeResponse);
        if (partialBody) {
          res.write('{"counts":');
        }
      });
      await assert.rejects(
        wait(url, { timeoutMs: 250 }),
        /deadline exceeded.*no response received/,
      );
      await closed;
    },
  );
}

test(
  'HTTP failures are reported immediately instead of being retried indefinitely',
  { timeout: 5_000 },
  async (t) => {
    const url = await server(t, (_req, res) => {
      res.writeHead(503);
      res.end();
    });
    await assert.rejects(wait(url), /SMTP acceptance:.*HTTP 503/);
  },
);

test(
  'missing inbox message expires even when the inbox keeps responding',
  { timeout: 5_000 },
  async (t) => {
    const url = await server(t, (_req, res) => res.end(JSON.stringify({ messages: [] })));
    await assert.rejects(
      until(
        (signal) => fetchJson(url, { signal }),
        (value) => value.messages.length > 0,
        'matching inbox message',
        { timeoutMs: 250, intervalMs: 5, log: () => {} },
      ),
      /matching inbox message: deadline exceeded.*"messages":\[\]/,
    );
  },
);
