import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

const workflow = readFileSync(new URL('../workflows/demo.yml', import.meta.url), 'utf8').replaceAll(
  '\r\n',
  '\n',
);
const gate = workflow.match(/node --input-type=module <<'NODE'\n([\s\S]*?)\n\s*NODE/)[1];

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
