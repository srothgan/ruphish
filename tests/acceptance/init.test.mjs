import test from 'node:test';
import assert from 'node:assert/strict';
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

test('fresh fixture initialization owns Cargo output and preserves existing artifacts and credentials', (t) => {
  const prefix = join(tmpdir(), 'ruphish-fixture-init-');
  const root = mkdtempSync(prefix);
  t.after(() => {
    assert(root.startsWith(prefix));
    rmSync(root, { recursive: true, force: true });
  });
  const fixture = join(root, 'tests', 'acceptance');
  mkdirSync(fixture, { recursive: true });
  const script = join(fixture, 'lab.mjs');
  copyFileSync(new URL('./lab.mjs', import.meta.url), script);
  const initialize = () => {
    const env = { ...process.env };
    delete env.RUPHISH_ACCEPTANCE_DATA_DIRECTORY;
    const result = spawnSync(process.execPath, [script, 'init'], {
      cwd: root,
      env,
      encoding: 'utf8',
    });
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stderr);
  };

  initialize();
  const configuration = JSON.parse(readFileSync(join(fixture, 'data', 'config.json'), 'utf8'));
  assert.equal(
    configuration.logging.filename,
    '',
    'Container logs belong to the configured Docker driver',
  );
  for (const path of [join(root, 'target'), join(root, 'target', 'docker')]) {
    assert(
      statSync(path).isDirectory(),
      'Artifact directories must exist before Docker binds them',
    );
    if (process.getuid) {
      assert.equal(statSync(path).uid, process.getuid());
    }
  }
  mkdirSync(join(root, 'target', 'debug'));
  const artifact = join(root, 'target', 'docker', 'ruphish');
  writeFileSync(artifact, 'existing artifact');
  const credentials = readFileSync(join(fixture, 'data', 'bootstrap.env'));
  initialize();
  assert.equal(readFileSync(artifact, 'utf8'), 'existing artifact');
  assert.deepEqual(readFileSync(join(fixture, 'data', 'bootstrap.env')), credentials);
});

test('isolated fixture state leaves the default lab data untouched', (t) => {
  const prefix = join(tmpdir(), 'ruphish-fixture-isolation-');
  const root = mkdtempSync(prefix);
  t.after(() => {
    assert(root.startsWith(prefix));
    rmSync(root, { recursive: true, force: true });
  });
  const fixture = join(root, 'tests', 'acceptance');
  const original = join(fixture, 'data');
  mkdirSync(original, { recursive: true });
  writeFileSync(join(original, 'bootstrap.env'), 'existing lab credentials');
  writeFileSync(join(original, 'config.json'), 'existing lab configuration');
  const script = join(fixture, 'lab.mjs');
  copyFileSync(new URL('./lab.mjs', import.meta.url), script);
  const isolated = join(root, 'disposable');
  const result = spawnSync(process.execPath, [script, 'init'], {
    cwd: root,
    env: { ...process.env, RUPHISH_ACCEPTANCE_DATA_DIRECTORY: isolated },
    encoding: 'utf8',
  });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(join(original, 'bootstrap.env'), 'utf8'), 'existing lab credentials');
  assert.equal(readFileSync(join(original, 'config.json'), 'utf8'), 'existing lab configuration');
  assert.equal(
    JSON.parse(readFileSync(join(isolated, 'config.json'))).db_path,
    '/lab/data/ruphish.db',
  );
  assert.match(
    readFileSync(join(isolated, 'bootstrap.env'), 'utf8'),
    /GOPHISH_INITIAL_ADMIN_PASSWORD=/,
  );
});
