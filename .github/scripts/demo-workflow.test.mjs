import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

const read = (name) =>
  readFileSync(new URL(`../workflows/${name}.yml`, import.meta.url), 'utf8').replaceAll(
    '\r\n',
    '\n',
  );
const workflow = read('demo');
const gate = workflow.match(/node --input-type=module <<'NODE'\n([\s\S]*?)\n\s*NODE/)[1];
const jobBodies = (yaml) =>
  Object.fromEntries(
    [...yaml.matchAll(/^ {2}([\w-]+):\n([\s\S]*?)(?=^ {2}[\w-]+:|$(?![\s\S]))/gm)]
      .filter(([, name]) => name !== 'workflow_call')
      .map(([, name, body]) => [name, body]),
  );

function check(selected, build, paths = 'success') {
  runInNewContext(gate, {
    process: {
      env: {
        RESULTS: JSON.stringify({
          changes: { result: paths, outputs: { demo: selected } },
          build: { result: build },
        }),
      },
    },
  });
}

test('demo workflow contains only selection, build and gate with one artifact producer', () => {
  const jobs = jobBodies(workflow);
  assert.deepEqual(Object.keys(jobs), ['changes', 'build', 'gate']);
  assert.match(jobs.changes, /filters: \.github\/paths\.yml/);
  assert.match(jobs.build, /if: needs\.changes\.outputs\.demo == 'true'/);
  assert.match(jobs.build, /trunk build --locked --release --features demo/);
  assert.match(jobs.build, /name: demo-vercel-output/);
  assert.doesNotMatch(
    read('validation'),
    /suite:|\n {2}demo:|name: demo-vercel-output|demo_changed:/,
  );
  assert.doesNotMatch(jobs.changes, /Check gate behavior|go run.*actionlint/);
});

test('demo gate accepts only successful selected builds and intentionally unselected builds', () => {
  check('true', 'success');
  check('false', 'skipped');
  for (const selected of ['true', 'false']) {
    for (const result of ['failure', 'cancelled', 'skipped', 'success']) {
      if (result === (selected === 'true' ? 'success' : 'skipped')) {
        continue;
      }
      assert.throws(() => check(selected, result), /Demo build/);
    }
  }
  for (const result of ['failure', 'cancelled', 'skipped']) {
    assert.throws(() => check('true', 'success', result), /Demo path selection/);
  }
  for (const output of [undefined, '', 'yes']) {
    assert.throws(() => check(output, 'skipped'), /Missing demo path output/);
  }
});

test('demo selection respects the canonical filter and full runs bypass it', () => {
  const expression = workflow.match(/ {6}demo: \$\{\{ (.*?) \}\}/)[1];
  for (const full of [false, true]) {
    for (const demo of ['true', 'false', undefined]) {
      const selected = runInNewContext(expression, {
        inputs: { full },
        steps: { paths: { outputs: { demo } } },
      });
      assert.equal(selected, full ? 'true' : demo);
    }
  }
});

test('PR, nightly and release reuse the same demo workflow and keep gate coverage', () => {
  for (const name of ['pr', 'nightly', 'release']) {
    const jobs = jobBodies(read(name));
    assert.match(jobs.demo, /uses: \.\/\.github\/workflows\/demo\.yml/);
    if (name !== 'pr') {
      assert.match(jobs.demo, /full: true/);
    }
    if (name !== 'nightly') {
      assert.match(jobs.gate, /needs: \[[^\]]*\bdemo\b/);
    }
  }
  assert.match(jobBodies(read('release')).assets, /needs: \[metadata, source, demo\]/);
});
