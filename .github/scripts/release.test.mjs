import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { runInNewContext } from 'node:vm';
import {
  combineImages,
  compareVersions,
  hash,
  parseVersion,
  readJson,
  releaseNotes,
  run,
  writeJson,
} from './release.mjs';
import { listReleases, publishRelease, seal } from './release-publish.mjs';

function temporary(t) {
  const root = mkdtempSync(join(tmpdir(), 'ruphish-release-test-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  return root;
}

test('only numbered dev/rc versions and final versions are publishable', () => {
  for (const version of ['0.1.0-dev.1', '0.1.0-rc.12', '0.1.0', '12.20.3']) {
    assert.equal(parseVersion(version).version, version);
  }
  for (const version of [
    'v0.1.0',
    '0.1.0-dev',
    '0.1.0-dev.0',
    '0.1.0-rc.01',
    '0.1.0-beta.1',
    '01.1.0',
    '0.1.0+build',
    '0.1.0\n',
    '0.1.0;echo secret',
  ]) {
    assert.throws(() => parseVersion(version), /Unsupported/);
  }
  assert(compareVersions('0.1.0-dev.10', '0.1.0-dev.2') > 0);
  assert(compareVersions('0.1.0-rc.1', '0.1.0-dev.99') > 0);
  assert(compareVersions('0.1.0', '0.1.0-rc.99') > 0);
  assert(compareVersions('0.10.0', '0.9.9') > 0);
});

test('prerelease notes stay short and point to the tagged changelog', () => {
  const dev = releaseNotes('0.1.0-dev.1', '## Unreleased\nDraft changes.', 'example/ruphish');
  assert.match(dev, /Development preview 1 for Ruphish 0\.1\.0/);
  assert.match(dev, /blob\/v0\.1\.0-dev\.1\/CHANGELOG.md/);
  assert.match(dev, /not guaranteed/);
  assert(!dev.includes('Changes since'));
  assert(!dev.includes('Draft changes.'));
  const rc = releaseNotes('0.1.0-rc.2', '', 'example/ruphish', '0.1.0-rc.1');
  assert.match(rc, /Release candidate 2/);
  assert.match(rc, /compare\/v0.1.0-rc.1\.\.\.v0.1.0-rc.2/);
  assert.throws(() => releaseNotes('0.1.0', '', 'example/ruphish', '0.2.0'));
});

test('final notes require exactly one complete dated version section', () => {
  const changelog =
    '# Changelog\n\n## Unreleased\nFuture change.\n\n## 0.1.0 - 2026-10-30\n\nInitial release.\n\n## 0.0.1 - 2026-01-01\nOld change.\n';
  const notes = releaseNotes('0.1.0', changelog, 'example/ruphish');
  assert.match(notes, /^Initial release\./);
  assert(!notes.includes('Future change'));
  assert(!notes.includes('Old change'));
  assert.throws(() => releaseNotes('0.2.0', changelog, 'example/ruphish'), /dated changelog/);
  assert.throws(
    () =>
      releaseNotes('0.1.0', changelog + '\n## 0.1.0 - 2026-10-30\nDuplicate.', 'example/ruphish'),
    /dated changelog/,
  );
  assert.throws(
    () => releaseNotes('0.1.0', '## 0.1.0 - 2026-10-30\nTODO\n', 'example/ruphish'),
    /complete/,
  );
});

test('metadata checks real Git tag identity, main ancestry and manual non-publication', (t) => {
  const directory = temporary(t);
  const git = (...args) => run('git', args, { cwd: directory });
  git('init', '-b', 'main');
  git('config', 'user.name', 'Release fixture');
  git('config', 'user.email', 'fixture@example.test');
  writeFileSync(join(directory, 'Cargo.toml'), '[workspace.package]\nversion = "0.1.0-dev.1"\n');
  writeFileSync(join(directory, 'CHANGELOG.md'), '## Unreleased\nInitial release.\n');
  git('add', '.');
  git('commit', '-m', 'Fixture');
  const commit = git('rev-parse', 'HEAD');
  git('update-ref', 'refs/remotes/origin/main', commit);
  git('tag', '-a', 'v0.1.0-dev.1', '-m', 'Preview');
  const script = resolve('.github/scripts/release.mjs');
  const invoke = (overrides = {}) =>
    spawnSync(process.execPath, [script, 'metadata'], {
      cwd: directory,
      encoding: 'utf8',
      env: {
        ...process.env,
        GITHUB_EVENT_NAME: 'push',
        GITHUB_SHA: commit,
        GITHUB_REF: 'refs/tags/v0.1.0-dev.1',
        GITHUB_REPOSITORY: 'example/ruphish',
        GITHUB_OUTPUT: '',
        ...overrides,
      },
    });
  assert.equal(invoke().status, 0);
  assert.equal(readJson(join(directory, 'target/release-metadata/release.json')).publish, true);
  assert.notEqual(invoke({ GITHUB_REF: 'refs/tags/v0.2.0' }).status, 0);
  assert.notEqual(invoke({ GITHUB_EVENT_NAME: 'pull_request' }).status, 0);
  assert.equal(
    invoke({ GITHUB_EVENT_NAME: 'workflow_dispatch', GITHUB_REF: 'refs/heads/main' }).status,
    0,
  );
  assert.equal(readJson(join(directory, 'target/release-metadata/release.json')).publish, false);
  writeFileSync(join(directory, 'later'), 'not merged');
  git('add', 'later');
  git('commit', '-m', 'Unmerged');
  git('tag', '-d', 'v0.1.0-dev.1');
  git('tag', '-a', 'v0.1.0-dev.1', '-m', 'Unmerged preview');
  assert.notEqual(invoke({ GITHUB_SHA: git('rev-parse', 'HEAD') }).status, 0);
});

function oci(directory, architecture) {
  mkdirSync(join(directory, 'blobs/sha256'), { recursive: true });
  const blob = (value, mediaType) => {
    const bytes = Buffer.from(JSON.stringify(value));
    const digest = `sha256:${hash(bytes)}`;
    writeFileSync(join(directory, 'blobs/sha256', digest.slice(7)), bytes);
    return { mediaType, digest, size: bytes.length };
  };
  const config = blob(
    {
      architecture,
      os: 'linux',
      config: {
        Labels: {
          'org.opencontainers.image.version': '0.1.0-dev.1',
          'org.opencontainers.image.revision': 'abc',
        },
      },
    },
    'application/vnd.oci.image.config.v1+json',
  );
  const layer = blob('original image layer', 'application/vnd.oci.image.layer.v1.tar+gzip');
  const manifest = blob(
    {
      schemaVersion: 2,
      mediaType: 'application/vnd.oci.image.manifest.v1+json',
      config,
      layers: [layer],
    },
    'application/vnd.oci.image.manifest.v1+json',
  );
  writeJson(join(directory, 'index.json'), { schemaVersion: 2, manifests: [manifest] });
  return { directory, architecture, version: '0.1.0-dev.1', commit: 'abc' };
}

test('multi-architecture assembly preserves image bytes and rejects corruption or wrong platform', (t) => {
  const directory = temporary(t);
  const inputs = ['amd64', 'arm64'].map((arch) => oci(join(directory, arch), arch));
  const output = join(directory, 'combined');
  const digest = combineImages(inputs, output);
  const bytes = readFileSync(join(output, 'blobs/sha256', digest.slice(7)));
  assert.equal(`sha256:${hash(bytes)}`, digest);
  const combined = JSON.parse(bytes);
  assert.deepEqual(
    combined.manifests.map((m) => m.platform),
    [
      { os: 'linux', architecture: 'amd64' },
      { os: 'linux', architecture: 'arm64' },
    ],
  );
  assert.equal(combined.annotations['org.opencontainers.image.version'], '0.1.0-dev.1');
  assert.throws(() =>
    combineImages(
      [inputs[0], { ...inputs[1], architecture: 'amd64' }],
      join(directory, 'bad-platform'),
    ),
  );
  const descriptor = readJson(join(inputs[0].directory, 'index.json')).manifests[0];
  writeFileSync(join(inputs[0].directory, 'blobs/sha256', descriptor.digest.slice(7)), 'tampered');
  assert.throws(() => combineImages(inputs, join(directory, 'corrupt')), /digest mismatch/);
});

test('checksums cover final package and provenance bytes', (t) => {
  const directory = temporary(t);
  mkdirSync(join(directory, 'assets'));
  writeFileSync(join(directory, 'assets/package.zip'), 'exact archive');
  writeFileSync(join(directory, 'assets/native-provenance.jsonl'), 'signed provenance');
  writeJson(join(directory, 'assets/release-manifest.json'), { version: '0.1.0-dev.1' });
  seal(directory);
  const manifest = readJson(join(directory, 'assets/release-manifest.json'));
  assert.deepEqual(Object.keys(manifest.files).sort(), [
    'SHA256SUMS',
    'native-provenance.jsonl',
    'package.zip',
  ]);
  for (const [name, digest] of Object.entries(manifest.files)) {
    assert.equal(digest, hash(readFileSync(join(directory, 'assets', name))));
  }
  assert.equal(
    readFileSync(join(directory, 'assets/SHA256SUMS'), 'utf8'),
    `${hash(Buffer.from('signed provenance'))}  native-provenance.jsonl\n${hash(Buffer.from('exact archive'))}  package.zip\n`,
  );
});

test('archive packaging round-trips executable, notices and build identity with spaced paths', (t) => {
  const root = temporary(t);
  const directory = join(root, 'checkout with spaces');
  mkdirSync(join(directory, 'dist/admin'), { recursive: true });
  mkdirSync(join(directory, 'target/release-metadata'), { recursive: true });
  writeJson(join(directory, 'target/release-metadata/release.json'), {
    version: '0.1.0-dev.1',
    commit: 'fixture-commit',
  });
  for (const name of ['LICENSE', 'THIRD-PARTY-NOTICES.md', 'dist/admin/THIRD-PARTY-NOTICES.txt']) {
    writeFileSync(join(directory, name), `Notice: ${name}\n`);
  }
  writeFileSync(join(directory, 'dist/admin/asset-manifest.json'), '{"ui.wasm":"fixture"}');
  const binary = join(directory, 'input executable');
  const bytes = Buffer.from([0, 255, 1, 128, 65]);
  writeFileSync(binary, bytes);
  const output = join(root, 'environment');
  const target =
    process.platform === 'win32' ? 'x86_64-pc-windows-msvc' : 'x86_64-unknown-linux-gnu';
  const result = spawnSync(
    process.execPath,
    [resolve('.github/scripts/release.mjs'), 'package', target, binary],
    {
      cwd: directory,
      encoding: 'utf8',
      env: { ...process.env, TMPDIR: root, TEMP: root, TMP: root, GITHUB_ENV: output },
    },
  );
  assert.equal(result.status, 0, result.stderr);
  const executable = readFileSync(output, 'utf8').trim().replace('RUPHISH_TEST_BINARY=', '');
  assert.deepEqual(readFileSync(executable), bytes);
  const build = readJson(join(executable, '..', 'build.json'));
  assert.equal(build.executable_sha256, hash(bytes));
  assert.equal(build.commit, 'fixture-commit');
  assert.equal(build.target, target);
  const extension = process.platform === 'win32' ? 'zip' : 'tar.gz';
  const archive = readFileSync(
    join(directory, 'target/release-packages', `ruphish-0.1.0-dev.1-${target}.${extension}`),
  );
  assert.deepEqual(
    archive.subarray(0, 2),
    process.platform === 'win32' ? Buffer.from('PK') : Buffer.from([0x1f, 0x8b]),
  );
});

function publication(version = '0.1.0-dev.1') {
  const manifest = {
    ...parseVersion(version),
    tag: `v${version}`,
    commit: 'abc',
    publish: true,
    image_digest: 'sha256:verified',
  };
  const files = {
    'release-manifest.json': Buffer.from(JSON.stringify(manifest)),
    'package.zip': Buffer.from('verified archive'),
  };
  const state = { release: null, assets: {}, image: null, events: [], finals: [], fail: null };
  const event = (name) => {
    state.events.push(name);
    if (state.fail === name) {
      throw new Error(`Interrupted ${name}`);
    }
  };
  const remote = {
    tagCommit: async () => 'abc',
    findRelease: async () => state.release,
    asset: async (_release, name) => state.assets[name] ?? null,
    imageDigest: async () => state.image,
    createDraft: async (_meta, notes) => {
      event('draft');
      state.release = {
        id: 1,
        draft: true,
        body: notes,
        target_commitish: manifest.commit,
        assets: [],
      };
      return state.release;
    },
    upload: async (_release, name, bytes) => {
      event(`upload:${name}`);
      state.assets[name] = bytes;
    },
    pushImage: async () => {
      event('image');
      state.image = manifest.image_digest;
    },
    verifyPublicImage: async () => {
      event('public-image');
    },
    listReleases: async () => state.finals,
    publish: async (_release, prerelease, latest) => {
      event('publish');
      state.release.draft = false;
      state.release.prerelease = prerelease;
      state.latestRelease = latest;
    },
    advanceLatest: async () => {
      event('latest');
    },
  };
  return {
    manifest,
    files,
    state,
    remote,
    execute: () => publishRelease(manifest, 'Release notes', files, remote),
  };
}

test('dev and RC publish all artifacts before release visibility and never advance latest', async () => {
  for (const version of ['0.1.0-dev.1', '0.1.0-rc.1']) {
    const p = publication(version);
    await p.execute();
    assert.deepEqual(p.state.events, [
      'draft',
      'upload:release-manifest.json',
      'upload:package.zip',
      'image',
      'public-image',
      'publish',
    ]);
    assert.equal(p.state.release.prerelease, true);
    assert.equal(p.state.latestRelease, false);
  }
});

test('final releases advance latest after publication without moving it backward', async () => {
  const p = publication('0.2.0');
  await p.execute();
  assert.equal(p.state.release.prerelease, false);
  assert.equal(p.state.latestRelease, true);
  assert.deepEqual(p.state.events.slice(-2), ['publish', 'latest']);
  const older = publication('0.1.9');
  older.state.finals = [{ tag_name: 'v0.2.0', draft: false, prerelease: false }];
  await older.execute();
  assert.equal(older.state.latestRelease, false);
  assert(!older.state.events.includes('latest'));
});

test('interrupted uploads and registry/publication failures resume matching retained artifacts', async () => {
  for (const failure of [
    'upload:release-manifest.json',
    'upload:package.zip',
    'image',
    'public-image',
    'publish',
    'latest',
  ]) {
    const p = publication('0.1.0');
    p.state.fail = failure;
    await assert.rejects(p.execute(), /Interrupted/);
    assert.equal(p.state.release.draft, failure !== 'latest');
    p.state.fail = null;
    p.state.events = [];
    await p.execute();
    assert.equal(p.state.release.draft, false);
    if (failure !== 'upload:release-manifest.json') {
      assert(!p.state.events.includes('upload:release-manifest.json'));
    }
    if (['publish', 'latest'].includes(failure)) {
      assert(!p.state.events.includes('image'));
    }
  }
});

test('conflicting artifacts, missing published bytes, changed tags and manual runs fail closed', async () => {
  const p = publication();
  await p.execute();
  p.files['package.zip'] = Buffer.from('different build');
  await assert.rejects(p.execute(), /asset differs/);
  delete p.state.assets['package.zip'];
  await assert.rejects(p.execute(), /never regenerate/);
  const image = publication();
  image.state.image = 'sha256:different';
  await assert.rejects(image.execute(), /different content/);
  assert.deepEqual(image.state.events, []);
  const manual = publication();
  manual.manifest.publish = false;
  await assert.rejects(manual.execute(), /Manual runs/);
  assert.deepEqual(manual.state.events, []);
  const moved = publication();
  moved.remote.tagCommit = async () => 'def';
  await assert.rejects(moved.execute(), /Tag moved/);
  assert.deepEqual(moved.state.events, []);
});

test('lost acknowledgements do not duplicate completed uploads, image pushes or publication', async () => {
  for (const operation of ['upload', 'pushImage', 'publish']) {
    const p = publication('0.1.0');
    const original = p.remote[operation];
    p.remote[operation] = async (...args) => {
      await original(...args);
      throw new Error('Response lost after remote success');
    };
    await assert.rejects(p.execute(), /Response lost/);
    p.remote[operation] = original;
    await p.execute();
    assert.equal(p.state.release.draft, false);
    assert.equal(p.state.events.filter((event) => event === 'draft').length, 1);
    assert.equal(
      p.state.events.filter((event) => event === 'upload:release-manifest.json').length,
      1,
    );
    assert.equal(p.state.events.filter((event) => event === 'image').length, 1);
    assert.equal(p.state.events.filter((event) => event === 'publish').length, 1);
  }
});

test('release gate rejects missing, failed, cancelled and skipped dependencies', () => {
  const workflow = readFileSync(
    new URL('../workflows/release.yml', import.meta.url),
    'utf8',
  ).replaceAll('\r\n', '\n');
  const gate = workflow.match(/node --input-type=module <<'NODE'\n([\s\S]*?)\n\s*NODE/)[1];
  const jobs = ['metadata', 'source', 'demo', 'assets', 'native', 'image', 'bundle'];
  const check = (results) =>
    runInNewContext(gate, { process: { env: { RESULTS: JSON.stringify(results) } } });
  const success = Object.fromEntries(jobs.map((job) => [job, { result: 'success' }]));
  check(success);
  for (const job of jobs) {
    for (const result of ['failure', 'cancelled', 'skipped', undefined]) {
      assert.throws(() => check({ ...success, [job]: result ? { result } : undefined }));
    }
  }
});

test('actual publication condition excludes manual runs, branch pushes and failed gates', () => {
  const workflow = readFileSync(new URL('../workflows/release.yml', import.meta.url), 'utf8');
  const condition = workflow.split('\n  publish:')[1].match(/\n {4}if: (.+)/)[1];
  const allowed = (event, ref, publish, gate) =>
    runInNewContext(condition, {
      github: { event_name: event, ref },
      needs: { metadata: { outputs: { publish } }, gate: { result: gate } },
      startsWith: (value, prefix) => value.startsWith(prefix),
    });
  assert.equal(allowed('push', 'refs/tags/v0.1.0-dev.1', 'true', 'success'), true);
  assert.equal(allowed('workflow_dispatch', 'refs/tags/v0.1.0', 'true', 'success'), false);
  assert.equal(allowed('push', 'refs/heads/main', 'true', 'success'), false);
  assert.equal(allowed('push', 'refs/tags/v0.1.0', 'false', 'success'), false);
  for (const result of ['failure', 'skipped', 'cancelled']) {
    assert.equal(allowed('push', 'refs/tags/v0.1.0', 'true', result), false);
  }
});

test('release lookup paginates and retains draft entries for resumption', async () => {
  const calls = [];
  const draft = { tag_name: 'v0.1.0-dev.1', draft: true };
  const releases = await listReleases(async (path) => {
    calls.push(path);
    return path.endsWith('page=1')
      ? Array.from({ length: 100 }, () => ({ draft: false }))
      : [draft];
  });
  assert.deepEqual(calls, ['/releases?per_page=100&page=1', '/releases?per_page=100&page=2']);
  assert.equal(releases.length, 101);
  assert.equal(releases[100], draft);
});
