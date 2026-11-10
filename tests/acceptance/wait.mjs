import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';

export async function fetchJson(url, options = {}) {
  const deadline = AbortSignal.timeout(10_000);
  const signal = options.signal ? AbortSignal.any([options.signal, deadline]) : deadline;
  const response = await fetch(url, { ...options, signal });
  assert(response.ok, `${options.method ?? 'GET'} ${url}: HTTP ${response.status}`);
  return response.json();
}

// Observers must pass the signal through to I/O so expiry cancels the request,
// including a response whose headers arrive but whose body never completes.
export async function until(
  observe,
  predicate,
  label,
  { timeoutMs = 60_000, intervalMs = 250, describe = JSON.stringify, log = console.log } = {},
) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  let last = 'no response received';
  let nextLog = performance.now();
  log(`Waiting for ${label} (deadline ${timeoutMs} ms)`);
  try {
    while (true) {
      controller.signal.throwIfAborted();
      const value = await observe(controller.signal);
      controller.signal.throwIfAborted();
      last = describe(value);
      if (predicate(value)) {
        log(`Completed ${label}`);
        return value;
      }
      if (performance.now() >= nextLog) {
        log(`Waiting for ${label}: ${last}`);
        nextLog = performance.now() + 5_000;
      }
      await delay(intervalMs, undefined, { signal: controller.signal });
    }
  } catch (error) {
    const reason = controller.signal.aborted
      ? `deadline exceeded after ${timeoutMs} ms`
      : error.message;
    throw new Error(`${label}: ${reason}. Last observation: ${last}`, { cause: error });
  } finally {
    clearTimeout(timer);
  }
}

export function deliverySummary(value) {
  return JSON.stringify({
    campaign_id: value.campaign.id,
    status: value.campaign.status,
    domain_block: value.campaign.domain_block,
    mailer_enabled: value.mailer_enabled,
    counts: value.counts,
    items: value.items.map(({ id, state, code, attempt_count, attempts }) => ({
      id,
      state,
      code,
      attempt_count,
      attempts: attempts.map(({ state, stage, smtp_code }) => ({ state, stage, smtp_code })),
    })),
  });
}

export function deliveryAccepted(value) {
  for (const state of ['blocked', 'failed', 'unknown', 'cancelled']) {
    assert(!(value.counts[state] > 0), `delivery entered ${state}`);
  }
  if (value.counts.accepted === 1) {
    return true;
  }
  assert(value.mailer_enabled, 'delivery workers are disabled');
  assert(!value.campaign.domain_block, `domain blocked: ${value.campaign.domain_block}`);
  assert.notEqual(value.campaign.status, 'Completed', 'campaign completed without SMTP acceptance');
  return false;
}
