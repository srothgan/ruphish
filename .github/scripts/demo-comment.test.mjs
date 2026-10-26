import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

const workflow = readFileSync(new URL('../workflows/pr.yml', import.meta.url), 'utf8');
const job = workflow.slice(workflow.indexOf('\n  demo-comment:'));
const guard = job.match(/    if: >-\r?\n([\s\S]*?)\r?\n    runs-on:/)[1].trim();
const script = job.match(/          script: \|\r?\n([\s\S]*)/)[1];
const marker = '<!-- ruphish-demo-preview -->';
const head = 'a'.repeat(40);

function eligible({ event = 'pull_request', fork = false, bot = false, enabled = 'true', changed = 'true', build = 'success', deploy = 'success', cancelled = false } = {}) {
  return runInNewContext(guard, {
    always: () => true, cancelled: () => cancelled,
    github: { event_name: event, repository: 'owner/repo', event: { pull_request: {
      head: { repo: { full_name: fork ? 'fork/repo' : 'owner/repo' } },
      user: { login: bot ? 'dependabot[bot]' : 'maintainer' },
    } } },
    vars: { DEMO_DEPLOY_ENABLED: enabled },
    needs: { demo: { result: build, outputs: { demo_changed: changed } }, 'deploy-demo': { result: deploy } },
  });
}

async function report({ build = 'success', deploy = 'success', comments = [], state = 'open', currentHead = head, url = 'https://ruphish-demo-example.vercel.app' } = {}) {
  const writes = [];
  const rest = {
    pulls: { get: async () => ({ data: { state, head: { sha: currentHead } } }) },
    issues: {
      listComments: () => {},
      createComment: async args => writes.push({ action: 'create', ...args }),
      updateComment: async args => writes.push({ action: 'update', ...args }),
    },
  };
  await runInNewContext(`(async () => { ${script}\n })()`, {
    github: { rest, paginate: async () => comments }, URL,
    context: { repo: { owner: 'owner', repo: 'repo' }, serverUrl: 'https://github.com', runId: 123,
      payload: { pull_request: { number: 28, head: { sha: head } } } },
    process: { env: { DEMO_RESULT: build, DEPLOY_RESULT: deploy, PREVIEW_URL: url } },
  });
  return writes;
}

test('report previews on success or failure without waiting for unrelated checks', () => {
  assert.equal(eligible(), true);
  assert.equal(eligible({ build: 'failure', deploy: 'skipped' }), true);
  assert.equal(eligible({ deploy: 'failure' }), true);
  for (const options of [{ event: 'push' }, { fork: true }, { bot: true }, { enabled: 'false' }, { changed: 'false' }, { cancelled: true }, { deploy: 'skipped' }]) {
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
  for (const [build, deploy, label] of [['failure', 'skipped', 'Build failed'], ['success', 'failure', 'Deployment failed']]) {
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
  for (const url of ['', 'http://example.vercel.app', 'https://example.com', 'https://example.vercel.app/extra']) {
    await assert.rejects(report({ url }));
  }
});
