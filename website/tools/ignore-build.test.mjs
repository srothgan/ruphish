import { test } from 'node:test';
import assert from 'node:assert/strict';
import { affectsWebsite, shouldBuild } from './ignore-build.mjs';

const env = { VERCEL_GIT_PREVIOUS_SHA: 'a'.repeat(40), VERCEL_GIT_COMMIT_SHA: 'b'.repeat(40) };
test('website and docs build; unrelated application changes do not', () => {
  for (const path of ['website/src/pages/index.astro', 'website/package-lock.json', 'docs/user-guide/index.md', 'website\\public\\logo.svg']) assert.equal(affectsWebsite([path]), true);
  assert.equal(affectsWebsite(['crates/ruphish-ui/src/overview.rs', 'README.md', 'other/docs/index.md', 'assets/dashboard.png']), false);
});
test('first build, missing history and malformed SHAs build conservatively', () => {
  assert.equal(shouldBuild({}, () => { throw new Error('must not run'); }), true);
  assert.equal(shouldBuild({ ...env, VERCEL_GIT_PREVIOUS_SHA: '--option' }, () => {}), true);
  assert.equal(shouldBuild(env, () => { throw new Error('shallow history'); }), true);
});
test('complete deployment diff covers multiple commits and both sides of renames', () => {
  assert.equal(shouldBuild(env, (args) => {
    assert.deepEqual(args, ['diff', '--name-only', '--no-renames', '-z', env.VERCEL_GIT_PREVIOUS_SHA, env.VERCEL_GIT_COMMIT_SHA, '--', 'website/', 'docs/']);
    return 'docs/removed page.md\0';
  }), true);
  assert.equal(shouldBuild(env, () => ''), false);
});
