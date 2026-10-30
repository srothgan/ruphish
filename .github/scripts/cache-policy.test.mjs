import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

const read = (path) =>
  readFileSync(new URL(path, import.meta.url), 'utf8').replaceAll('\r\n', '\n');
const workflows = Object.fromEntries(
  readdirSync(new URL('../workflows/', import.meta.url))
    .filter((name) => name.endsWith('.yml'))
    .map((name) => [name, read(`../workflows/${name}`)]),
);
const jobs = (workflow) =>
  Object.fromEntries(
    [...workflow.matchAll(/^ {2}([\w-]+):\n([\s\S]*?)(?=^ {2}[\w-]+:|$(?![\s\S]))/gm)]
      .filter(([, , body]) => /^ {4}(uses|runs-on):/m.test(body))
      .map(([, name, body]) => [name, body]),
  );
const mode = (text) => text.match(/^\s*cache-mode: (\S+)$/m)?.[1];
const rootMode = (workflow) => mode(workflow.split('\njobs:')[0]);
const script = (text) => text.match(/node --input-type=module <<'NODE'\n([\s\S]*?)\n\s*NODE/)[1];

function selected(workflow, job, github) {
  let guard = job.match(/^ {4}if: (.+)$/m)?.[1] ?? 'true';
  if (guard.startsWith('*')) {
    const anchor = guard.slice(1);
    guard = workflow.match(new RegExp(`if: &${anchor} (.+)`))[1];
  }
  guard = guard.replace(/^&[\w-]+ /, '').replace(/^\$\{\{ (.*) \}\}$/, '$1');
  return runInNewContext(guard, { github });
}

test('only main push, dispatch and nightly builds can populate shared caches', () => {
  const scenarios = [
    ['pr.yml', 'pull_request', 'refs/pull/43/merge', 'read'],
    ['pr.yml', 'push', 'refs/heads/main', 'write'],
    ['pr.yml', 'workflow_dispatch', 'refs/heads/main', 'write'],
    ['pr.yml', 'workflow_dispatch', 'refs/heads/feature', 'read'],
    ['pr.yml', 'workflow_dispatch', 'refs/tags/v0.1.0-dev.1', 'read'],
    ['nightly.yml', 'schedule', 'refs/heads/main', 'write'],
    ['nightly.yml', 'workflow_dispatch', 'refs/heads/main', 'write'],
    ['nightly.yml', 'workflow_dispatch', 'refs/heads/feature', 'read'],
  ];
  for (const [name, event_name, ref, expected] of scenarios) {
    const workflow = workflows[name];
    const calls = Object.values(jobs(workflow)).filter((job) => /^ {4}uses:/m.test(job));
    const active = calls.filter((job) => selected(workflow, job, { event_name, ref }));
    const suites = active.map((job) => job.match(/^ {4}uses: (.+)$/m)[1]);
    assert.equal(active.length, name === 'pr.yml' ? 2 : 1, `${event_name}: selected suites`);
    assert.equal(new Set(suites).size, suites.length, 'No duplicate suite executions');
    for (const job of active) {
      assert.equal(mode(job) ?? rootMode(workflow), expected, `${name}: ${ref}`);
    }
  }
});

test('cache access remains disabled throughout release and privileged publication jobs', () => {
  function check(workflow, inherited) {
    const effective = rootMode(workflow) ?? inherited;
    assert.equal(effective, 'none');
    for (const job of Object.values(jobs(workflow))) {
      const access = mode(job) ?? effective;
      assert.equal(access, 'none');
      const called = job.match(/^ {4}uses: \.\/\.github\/workflows\/(.+)$/m)?.[1];
      if (called) {
        check(workflows[called], access);
      }
    }
  }
  check(workflows['release.yml']);
  for (const job of Object.values(jobs(workflows['pr.yml']))) {
    if (/^ {6}(pull-requests|packages|id-token): write$/m.test(job)) {
      assert.equal(mode(job), 'none');
    }
  }
});

test('cache modes use supported literals at workflow or job scope', () => {
  for (const workflow of Object.values(workflows)) {
    for (const [, indent, value] of workflow.matchAll(/^( *)cache-mode: (.+)$/gm)) {
      assert.ok([0, 4].includes(indent.length));
      assert.ok(['read', 'write', 'none'].includes(value), value);
    }
  }
  // Reusable suites inherit the caller's token restriction, including release's none.
  for (const name of ['validation.yml', 'demo.yml']) {
    assert.equal(rootMode(workflows[name]), undefined);
  }
});

test('toolchain translates enforced token permissions and fails closed without them', () => {
  const source = script(read('../actions/toolchain/action.yml')).replace(
    /import .* from 'node:fs';/,
    '',
  );
  for (const [access, expected] of [
    ['read', 'restore=true\nsave=false\n'],
    ['write', 'restore=true\nsave=true\n'],
    ['none', 'restore=false\nsave=false\n'],
  ]) {
    const writes = [];
    runInNewContext(source, {
      process: { env: { ACTIONS_CACHE_MODE: access, GITHUB_OUTPUT: 'output' } },
      appendFileSync: (path, value) => writes.push([path, value]),
    });
    assert.deepEqual(writes, [['output', expected]]);
  }
  for (const access of [undefined, '', 'write-only', 'invalid']) {
    assert.throws(
      () =>
        runInNewContext(source, {
          process: { env: { ACTIONS_CACHE_MODE: access } },
          appendFileSync: () => assert.fail('Unexpected permission must not enable caching'),
        }),
      /Unexpected cache mode/,
    );
  }
});

test('CI gates reject failed, cancelled, missing or duplicate suite executions', () => {
  const ci = jobs(workflows['pr.yml']);
  for (const first of ['success', 'skipped', 'failure', 'cancelled', undefined]) {
    for (const second of ['success', 'skipped', 'failure', 'cancelled', undefined]) {
      const valid =
        (first === 'success' && second === 'skipped') ||
        (first === 'skipped' && second === 'success');
      const demo = () =>
        runInNewContext(script(ci.demo), {
          process: {
            env: { RESULTS: JSON.stringify({ read: { result: first }, main: { result: second } }) },
          },
        });
      if (valid) {
        demo();
      } else {
        assert.throws(demo);
      }
      for (const result of ['success', 'skipped', 'failure', 'cancelled', undefined]) {
        const gate = () =>
          runInNewContext(script(ci.gate), {
            process: { env: { RESULT: first, MAIN_RESULT: second, DEMO_RESULT: result } },
          });
        if (valid && result === 'success') {
          gate();
        } else {
          assert.throws(gate);
        }
      }
    }
  }
});
