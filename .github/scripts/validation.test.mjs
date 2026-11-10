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
const groups = ['product', 'persistence', 'public'];
const jobs = ['assets', 'native', 'platforms', 'database', 'dependencies', 'public'];

function results(selected = [], succeeded = []) {
  return {
    changes: {
      result: 'success',
      outputs: Object.fromEntries(groups.map((group) => [group, String(selected.includes(group))])),
    },
    ...Object.fromEntries(
      jobs.map((job) => [
        job,
        { result: job === 'dependencies' || succeeded.includes(job) ? 'success' : 'skipped' },
      ]),
    ),
  };
}

function check(value) {
  runInNewContext(gate, { process: { env: { RESULTS: JSON.stringify(value) } } });
}

test('dependency job always runs the advisory and dependency policy checks', () => {
  const job = workflow.match(/^ {2}dependencies:\n([\s\S]*?)(?=^ {2}\w+:)/m)[1];
  assert.doesNotMatch(job, /^ {4}(?:if|needs):/m);
  assert.match(job, /run: cargo deny check advisories bans licenses sources\n/);
});

test('housekeeping and website changes can pass without product jobs', () => {
  check(results());
});

test('dependency policy cannot be skipped or fail even for housekeeping changes', () => {
  for (const status of ['failure', 'cancelled', 'skipped', undefined]) {
    const value = results();
    value.dependencies.result = status;
    assert.throws(() => check(value), /dependencies: expected success/);
  }
  const value = results();
  delete value.dependencies;
  assert.throws(() => check(value));
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
  for (const job of jobs.filter((job) => job !== 'dependencies')) {
    for (const status of ['success', 'failure', 'cancelled']) {
      const value = results();
      value[job].result = status;
      assert.throws(() => check(value), undefined, `${job}: ${status}`);
    }
  }
});
