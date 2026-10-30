import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

// Exercise the actual inline gate, including missing outputs and dependency skips.
const workflow = readFileSync(
  new URL('../workflows/validation.yml', import.meta.url),
  'utf8',
).replaceAll('\r\n', '\n');
const gate = workflow.match(/node --input-type=module <<'NODE'\n([\s\S]*?)\n\s*NODE/)[1];
const groups = ['product', 'persistence', 'dependencies', 'public'];
const jobs = ['assets', 'native', 'platforms', 'database', 'dependencies', 'public'];

function results(selected = [], succeeded = []) {
  return {
    changes: {
      result: 'success',
      outputs: Object.fromEntries(groups.map((group) => [group, String(selected.includes(group))])),
    },
    ...Object.fromEntries(
      jobs.map((job) => [job, { result: succeeded.includes(job) ? 'success' : 'skipped' }]),
    ),
  };
}

function check(value) {
  runInNewContext(gate, { process: { env: { RESULTS: JSON.stringify(value) } } });
}

test('housekeeping and website changes can pass without product jobs', () => {
  check(results());
});

test('product and complete runs require their fixed suites', () => {
  check(results(['product'], ['assets', 'native', 'platforms']));
  check(results(['product', 'persistence'], ['assets', 'native', 'platforms', 'database']));
  check(results(groups, jobs));
});

test('failed, cancelled and skipped selected jobs cannot pass', () => {
  for (const job of ['changes', ...jobs]) {
    for (const status of ['failure', 'cancelled', 'skipped']) {
      const value = results(groups, jobs);
      value[job].result = status;
      assert.throws(() => check(value), undefined, `${job}: ${status}`);
    }
  }
});

test('missing or malformed path outputs fail closed', () => {
  for (const group of groups) {
    for (const output of [undefined, '', 'yes']) {
      const value = results();
      value.changes.outputs[group] = output;
      assert.throws(() => check(value), /Missing path group/);
    }
  }
});

test('unexpected jobs cannot hide failures behind unselected groups', () => {
  for (const job of jobs) {
    for (const status of ['success', 'failure', 'cancelled']) {
      const value = results();
      value[job].result = status;
      assert.throws(() => check(value), undefined, `${job}: ${status}`);
    }
  }
});
