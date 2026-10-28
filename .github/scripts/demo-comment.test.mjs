import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

const workflow = readFileSync(new URL('../workflows/pr.yml', import.meta.url), 'utf8');
const job = workflow.slice(workflow.indexOf('\n  demo-comment:'));
const guard = job.match(/ {4}if: >-\r?\n([\s\S]*?)\r?\n {4}runs-on:/)[1].trim();
const script = workflow.match(
  / {10}script: &demo-comment-script \|\r?\n((?: {12}.*(?:\r?\n|$)|\r?\n)*)/,
)[1];
const marker = '<!-- ruphish-demo-preview -->';
const head = 'a'.repeat(40);

function eligible({
  event = 'pull_request',
  fork = false,
  bot = false,
  enabled = 'true',
  changed = 'true',
  build = 'success',
  deploy = 'success',
  cancelled = false,
} = {}) {
  return runInNewContext(guard, {
    always: () => true,
    cancelled: () => cancelled,
    github: {
      event_name: event,
      repository: 'owner/repo',
      event: {
        pull_request: {
          head: { repo: { full_name: fork ? 'fork/repo' : 'owner/repo' } },
          user: { login: bot ? 'dependabot[bot]' : 'maintainer' },
        },
      },
    },
    vars: { DEMO_DEPLOY_ENABLED: enabled },
    needs: {
      demo: { result: build, outputs: { demo_changed: changed } },
      'deploy-demo': { result: deploy },
    },
  });
}

async function report({
  phase = 'finished',
  build = 'success',
  deploy = 'success',
  comments = [],
  state = 'open',
  currentHead = head,
  runId = 123,
  attempt = '1',
  url = 'https://ruphish-demo-example.vercel.app',
} = {}) {
  const writes = [];
  const rest = {
    pulls: { get: async () => ({ data: { state, head: { sha: currentHead } } }) },
    issues: {
      listComments: () => {},
      createComment: async (args) => writes.push({ action: 'create', ...args }),
      updateComment: async (args) => writes.push({ action: 'update', ...args }),
    },
  };
  await runInNewContext(`(async () => { ${script}\n })()`, {
    github: { rest, paginate: async () => comments },
    URL,
    context: {
      repo: { owner: 'owner', repo: 'repo' },
      serverUrl: 'https://github.com',
      runId,
      payload: { pull_request: { number: 28, head: { sha: head } } },
    },
    process: {
      env: {
        REPORT_PHASE: phase,
        DEMO_RESULT: build,
        DEPLOY_RESULT: deploy,
        PREVIEW_URL: url,
        GITHUB_RUN_ATTEMPT: attempt,
      },
    },
  });
  return writes;
}

test('report previews on success or failure without waiting for unrelated checks', () => {
  assert.equal(eligible(), true);
  assert.equal(eligible({ build: 'failure', deploy: 'skipped' }), true);
  assert.equal(eligible({ deploy: 'failure' }), true);
  for (const options of [
    { event: 'push' },
    { fork: true },
    { bot: true },
    { enabled: 'false' },
    { changed: 'false' },
    { cancelled: true },
    { deploy: 'skipped' },
  ]) {
    assert.equal(eligible(options), false, JSON.stringify(options));
  }
});

test('successful preview creates a bot comment with deployment, commit and logs', async () => {
  const [comment] = await report();
  assert.equal(comment.action, 'create');
  assert.equal(comment.issue_number, 28);
  assert.match(comment.body, /✅ Ready/);
  assert.match(comment.body, /\[Visit preview\]\(https:\/\/ruphish-demo-example\.vercel\.app\/\)/);
  assert.ok(comment.body.includes(`/commit/${head}`));
  assert.ok(comment.body.includes('/actions/runs/123'));
});

test('failure updates only the matching Actions bot comment and removes a stale preview link', async () => {
  const comments = [
    { id: 1, user: { login: 'maintainer', type: 'User' }, body: marker },
    { id: 2, user: { login: 'vercel[bot]', type: 'Bot' }, body: marker },
    { id: 3, user: { login: 'github-actions[bot]', type: 'Bot' }, body: `${marker}\nOld success` },
  ];
  for (const [build, deploy, label] of [
    ['failure', 'skipped', 'Build failed'],
    ['success', 'failure', 'Deployment failed'],
  ]) {
    const [comment] = await report({ build, deploy, comments, url: '' });
    assert.equal(comment.action, 'update');
    assert.equal(comment.comment_id, 3);
    assert.ok(comment.body.includes(label));
    assert.ok(comment.body.includes('Unavailable'));
    assert.ok(!comment.body.includes('Visit preview'));
  }
});

test('old runs cannot overwrite the current PR preview or report on closed PRs', async () => {
  assert.deepEqual(await report({ currentHead: 'b'.repeat(40) }), []);
  assert.deepEqual(await report({ state: 'closed' }), []);
});

test('a success comment requires a valid Vercel deployment origin', async () => {
  for (const url of [
    '',
    'http://example.vercel.app',
    'https://example.com',
    'https://example.vercel.app/extra',
  ]) {
    await assert.rejects(report({ url }));
  }
});

test('one comment progresses from building through approval and deploying to a ready preview', async () => {
  const comments = [];
  for (const [phase, status] of [
    ['building', 'Building'],
    ['approval', 'Awaiting approval'],
    ['deploying', 'Deploying'],
    ['finished', 'Ready'],
  ]) {
    const [comment] = await report({ phase, comments });
    assert.equal(comment.action, comments.length ? 'update' : 'create');
    assert.ok(comment.body.includes(status));
    assert.equal(comment.body.includes('Visit preview'), phase === 'finished');
    if (comments.length) {
      assert.equal(comment.comment_id, 10);
    }
    comments.splice(0, comments.length, {
      id: 10,
      user: { login: 'github-actions[bot]', type: 'Bot' },
      body: comment.body,
    });
  }
});

test('new runs reset the previous preview link before building', async () => {
  const [ready] = await report();
  const [building] = await report({
    phase: 'building',
    runId: 124,
    comments: [{ id: 10, user: { login: 'github-actions[bot]', type: 'Bot' }, body: ready.body }],
  });
  assert.equal(building.action, 'update');
  assert.ok(building.body.includes('Building'));
  assert.ok(!building.body.includes('Visit preview'));
});

test('reporting stages are ordered without becoming build or deployment prerequisites', () => {
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
  assert.deepEqual(dependencies('demo'), []);
  assert.deepEqual(dependencies('deploy-demo'), ['demo']);
  assert.deepEqual(dependencies('demo-comment-start'), []);
  assert.deepEqual(dependencies('demo-comment-approval'), ['demo', 'demo-comment-start']);
  assert.deepEqual(dependencies('demo-comment'), [
    'demo',
    'deploy-demo',
    'demo-comment-start',
    'demo-comment-approval',
  ]);
  assert.match(jobs['demo-comment-start'], /filters: \.github\/paths\.yml/);
  assert.match(jobs['demo-comment-start'], /if: steps\.paths\.outputs\.demo == 'true'/);
  for (const name of ['demo-comment-start', 'demo-comment-approval', 'demo-comment']) {
    assert.match(jobs[name], /script: \*demo-comment-script/);
  }
  const deployment = jobs['deploy-demo'];
  const reporting = deployment.slice(
    deployment.indexOf('- name: Report demo deployment starting'),
    deployment.indexOf('# Deliberately no source checkout'),
  );
  assert.match(reporting, /if: github.event_name == 'pull_request'/);
  assert.match(reporting, /continue-on-error: true/);
  assert.match(reporting, /REPORT_PHASE: deploying/);
  assert.ok(deployment.indexOf('REPORT_PHASE: deploying') < deployment.indexOf('vercel deploy'));
});

test('late progress reports cannot replace an already recorded later stage', async () => {
  const [deploying] = await report({ phase: 'deploying' });
  const comments = [
    { id: 10, user: { login: 'github-actions[bot]', type: 'Bot' }, body: deploying.body },
  ];
  assert.deepEqual(await report({ phase: 'building', comments }), []);
  assert.deepEqual(await report({ phase: 'approval', comments }), []);
  const [ready] = await report({ comments });
  comments[0].body = ready.body;
  assert.deepEqual(await report({ phase: 'deploying', comments }), []);
  assert.deepEqual(await report({ runId: 122, comments }), []);
  assert.equal((await report({ phase: 'building', attempt: '2', comments }))[0].action, 'update');
});
