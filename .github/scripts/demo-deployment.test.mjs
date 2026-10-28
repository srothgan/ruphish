import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

// This guard uses operators shared by GitHub expressions and JavaScript.
// Evaluate the workflow's guard itself, rather than a copied policy function.
const workflow = readFileSync(new URL('../workflows/pr.yml', import.meta.url), 'utf8');
const guard = workflow.match(/ {4}if: >-\r?\n([\s\S]*?)\r?\n {4}runs-on:/)[1].trim();
const github = (
  event = 'pull_request',
  repository = 'owner/repo',
  login = 'maintainer',
  ref = 'refs/pull/1/merge',
) => ({
  event_name: event,
  ref,
  repository: 'owner/repo',
  event: { pull_request: { head: { repo: { full_name: repository } }, user: { login } } },
});
const allowed = (event, enabled = 'true', changed = 'true') =>
  runInNewContext(guard, {
    github: event,
    vars: { DEMO_DEPLOY_ENABLED: enabled },
    needs: { demo: { outputs: { demo_changed: changed } } },
  });

test('deployment requires explicit activation and a selected demo artifact', () => {
  assert.equal(allowed(github()), true);
  for (const value of ['', 'false', undefined]) {
    assert.equal(allowed(github(), value ?? ''), false);
    assert.equal(allowed(github(), 'true', value ?? ''), false);
  }
});

test('forks and Dependabot cannot enter the secret-bearing preview job', () => {
  assert.equal(allowed(github('pull_request', 'contributor/fork')), false);
  assert.equal(allowed(github('pull_request', 'owner/repo', 'dependabot[bot]')), false);
});

test('production is limited to main pushes and explicit main dispatches', () => {
  for (const event of ['push', 'workflow_dispatch']) {
    assert.equal(allowed(github(event, 'owner/repo', 'maintainer', 'refs/heads/main')), true);
    assert.equal(allowed(github(event, 'owner/repo', 'maintainer', 'refs/heads/feature')), false);
  }
  assert.equal(allowed(github('schedule', 'owner/repo', 'maintainer', 'refs/heads/main')), false);
});

test('deployment waits only for the demo, while the PR gate still requires both suites', () => {
  const jobs = Object.fromEntries(
    [...workflow.matchAll(/^ {2}([\w-]+):\r?\n([\s\S]*?)(?=^ {2}[\w-]+:|$(?![\s\S]))/gm)].map(
      ([, name, body]) => [name, body],
    ),
  );
  const dependencies = (name) =>
    jobs[name]
      .match(/^ {4}needs: (.+)$/m)?.[1]
      .trim()
      .replace(/[[\]]/g, '')
      .split(/,\s*/) ?? [];
  const ancestors = (name) =>
    dependencies(name).flatMap((parent) => [parent, ...ancestors(parent)]);
  assert.deepEqual(ancestors('deploy-demo'), ['demo']);
  assert.deepEqual(dependencies('gate').sort(), ['checks', 'demo']);
  assert.match(jobs.checks, /uses: \.\/.github\/workflows\/validation\.yml/);
  assert.match(jobs.demo, /uses: \.\/.github\/workflows\/demo\.yml/);
  assert.doesNotMatch(jobs.checks + jobs.demo, /suite:/);
  assert.match(jobs['deploy-demo'], /demo-preview.*demo-production/);
});
