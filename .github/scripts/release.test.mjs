import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { runInNewContext } from 'node:vm';
import { pathToFileURL } from 'node:url';
import {
  combineImages,
  compareVersions,
  enrichChanges,
  generateNotes,
  groupedChanges,
  hash,
  parseVersion,
  parseSubject,
  readHistory,
  selectBaseline,
  readJson,
  releaseNotes,
  run,
  writeJson,
} from './release.mjs';
import { listReleases, publishRelease, seal } from './release-publish.mjs';
import { github } from './github.mjs';

function temporary(t) {
  const root = mkdtempSync(join(tmpdir(), 'ruphish-release-test-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  return root;
}

test('only numbered development versions and final versions are publishable', () => {
  for (const version of ['0.1.0-dev.1', '0.1.0-dev.12', '0.1.0', '12.20.3']) {
    assert.equal(parseVersion(version).version, version);
  }
  for (const version of [
    'v0.1.0',
    '0.1.0-dev',
    '0.1.0-dev.0',
    '0.1.0-dev.01',
    '0.1.0-beta.1',
    '01.1.0',
    '0.1.0+build',
    '0.1.0\n',
    '0.1.0;echo secret',
  ]) {
    assert.throws(() => parseVersion(version), /Unsupported/);
  }
  assert(compareVersions('0.1.0-dev.10', '0.1.0-dev.2') > 0);
  assert(compareVersions('0.1.0', '0.1.0-dev.99') > 0);
  assert(compareVersions('0.10.0', '0.9.9') > 0);
});

const expectedGroups = [
  ['feat', 'Features'],
  ['fix', 'Fixes'],
  ['perf', 'Performance'],
  ['docs', 'Documentation'],
  ['refactor', 'Refactoring'],
  ['style', 'Code style'],
  ['test', 'Tests'],
  ['ci', 'CI'],
  ['chore', 'Maintenance'],
  ['revert', 'Reverts'],
];

function preview(changes, baseline = null, commit = 'f'.repeat(40)) {
  return releaseNotes('1.0.0-dev.3', '', 'example/ruphish', { commit, baseline, changes });
}

test('preview notes start with a warning, group every type and omit comparison and installation boilerplate', () => {
  const workflow = readFileSync(new URL('../workflows/pr-title.yml', import.meta.url), 'utf8');
  const types = workflow
    .match(/types: \|\r?\n([\s\S]*?)\r?\n\s+requireScope:/)[1]
    .trim()
    .split(/\s+/);
  assert.deepEqual(types.sort(), expectedGroups.map(([type]) => type).sort());
  const changes = [...expectedGroups].reverse().map(([type, name], i) => ({
    commit: (i + 1).toString(16).repeat(40),
    subject: `${type}: ${name} example (#${i + 1})`,
  }));
  const notes = preview(changes, { version: '1.0.0-dev.2', commit: 'e'.repeat(40) });
  assert.deepEqual(
    [...notes.matchAll(/^### (.+)$/gm)].map((match) => match[1]),
    expectedGroups.map(([, name]) => name),
  );
  for (const [i, [, name]] of [...expectedGroups].reverse().entries()) {
    const entry = notes.trimEnd().split(`### ${name}\n\n`)[1].split('\n\n')[0];
    const commit = (i + 1).toString(16).repeat(40);
    assert.equal(
      entry,
      `- ${name} example \\(#${i + 1}\\) ([${commit.slice(0, 7)}](https://github.com/example/ruphish/commit/${commit}))`,
    );
  }
  assert.ok(
    notes.startsWith(
      '> [!WARNING]\n> **Development preview 3 for Ruphish 1.0.0 - testing only.**\n> Use a disposable installation.',
    ),
  );
  assert.ok(
    notes.includes(
      'Upgrades between previews or to the final release are not guaranteed; a fresh installation may be required.',
    ),
  );
  assert.ok(notes.indexOf('> [!WARNING]') < notes.indexOf('### Features'));
  assert.equal(
    notes.slice(notes.indexOf('### Features')),
    `${groupedChanges('example/ruphish', changes)}\n`,
  );
  assert.doesNotMatch(
    notes,
    /```|docker pull|install\.sh|install\.ps1|Planned changes|CHANGELOG|Verification|Native binaries|\/compare\/|Changes since|installer reference/,
  );
});

test('subjects omit displayed scopes and retain breaking markers, Unicode, Markdown and historical references', () => {
  assert.deepEqual(parseSubject('feat(api)!: Rename endpoint (#42)'), {
    type: 'feat',
    scope: 'api',
    breaking: true,
    description: 'Rename endpoint (#42)',
  });
  assert.deepEqual(parseSubject('fix!: Reject invalid data'), {
    type: 'fix',
    scope: null,
    breaking: true,
    description: 'Reject invalid data',
  });
  const notes = preview([
    {
      commit: '1'.repeat(40),
      subject: 'fix(ui): Übergrößen & <script> *bold* `code` [link](url) #42',
    },
    { commit: '2'.repeat(40), subject: 'feat(api)!: Rename endpoint (#43)' },
    { commit: '3'.repeat(40), subject: 'Fix historical subjects (#44)' },
    { commit: '4'.repeat(40), subject: 'build(deps): refresh dependencies' },
    { commit: '5'.repeat(40), subject: '### heading | ~strike~ \\path' },
  ]);
  assert.deepEqual(
    [...notes.matchAll(/^### (.+)$/gm)].map((match) => match[1]),
    ['Features', 'Fixes', 'Other changes'],
  );
  assert.ok(notes.includes('**Breaking change:** Rename endpoint \\(#43\\)'));
  assert.doesNotMatch(notes, /\*\*api:|\*\*ui:|fix\(ui\)|feat\(api\)/);
  assert.ok(
    notes.includes('Übergrößen &amp; \\<script\\> \\*bold\\* \\`code\\` \\[link\\]\\(url\\) #42'),
  );
  assert.ok(notes.includes('Fix historical subjects \\(#44\\)'));
  assert.ok(notes.includes('build\\(deps\\): refresh dependencies'));
  assert.ok(notes.includes('\\#\\#\\# heading \\| \\~strike\\~ \\\\path'));
  assert.ok(!notes.includes('### Performance'));
  assert.ok(!notes.includes('Changes since'));
  assert.ok(!notes.includes('<script>'));
  assert.deepEqual([...preview([]).matchAll(/^### (.+)$/gm)], []);
});

test('final notes require exactly one complete dated version section', () => {
  const body =
    '### Features\n\n- Add shared resources ([#42](https://github.com/example/ruphish/pull/42)), by @alice.\n\nThanks to @alice and @bob for testing.';
  const changelog = `# Changelog\n\n## Unreleased\nFuture change.\n\n## [0.1.0] - 2026-10-30\n\n${body}\n\n## 0.0.1 - 2026-01-01\nOld change.\n`;
  const notes = releaseNotes('0.1.0', changelog, 'example/ruphish');
  assert.equal(notes, `${body}\n`);
  assert.equal(
    releaseNotes('0.1.0', changelog.replaceAll('\n', '\r\n'), 'example/ruphish'),
    `${body}\n`,
  );
  assert(!notes.includes('Future change'));
  assert(!notes.includes('Old change'));
  assert.throws(() => releaseNotes('0.2.0', changelog, 'example/ruphish'), /dated changelog/);
  assert.throws(
    () =>
      releaseNotes('0.1.0', changelog + '\n## 0.1.0 - 2026-10-30\nDuplicate.', 'example/ruphish'),
    /dated changelog/,
  );
  for (const entry of ['', '   \n', 'TODO', '- TODO: describe the change']) {
    assert.throws(
      () => releaseNotes('0.1.0', `## 0.1.0 - 2026-10-30\n${entry}\n`, 'example/ruphish'),
      /complete/,
    );
  }
});

test('final extraction matches the heading version and allows a trailing Changes link or title', () => {
  const body = '### Features\n\n- Authored change by [@alice](https://github.com/alice).';
  const changelog = `# Changelog\n\n## [0.1.0] - 2026-10-27 [Changes][v0.16.2]\n\n${body}\n\n## Unreleased\nDo not publish this.\n`;
  assert.equal(releaseNotes('0.1.0', changelog, 'example/ruphish'), `${body}\n`);
  assert.throws(() => releaseNotes('0.16.2', changelog, 'example/ruphish'), /dated changelog/);
  assert.throws(
    () =>
      releaseNotes(
        '0.1.0',
        changelog + '\n## 0.1.0 - 2026-11-10 Final release\nDuplicate.\n',
        'example/ruphish',
      ),
    /dated changelog/,
  );
  assert.throws(
    () =>
      releaseNotes(
        '0.1.0',
        '## [0.1.0] - 2026-10-27 [Changes][v0.16.2]\n\n## Unreleased\nOther text.\n',
        'example/ruphish',
      ),
    /complete/,
  );
});

test('GitHub enrichment links associated merged PRs and verified contributors without guessing', async () => {
  const commit = 'a'.repeat(40);
  const pull = (number, extra = {}) => ({
    number,
    merged_at: '2026-11-10T10:00:00Z',
    base: { repo: { full_name: 'example/ruphish' } },
    ...extra,
  });
  const calls = [];
  const [change] = await enrichChanges(
    [{ commit, subject: 'fix(ui): resolve conflict (#42), see #99' }],
    'example/ruphish',
    async (path) => {
      calls.push(path);
      if (path === `/commits/${commit}`) {
        return {
          sha: commit,
          author: { login: 'srothgan', html_url: 'https://github.com/srothgan' },
        };
      }
      if (path.endsWith('page=1')) {
        return [pull(42), ...Array.from({ length: 99 }, () => pull(90, { merged_at: null }))];
      }
      assert.equal(path, `/commits/${commit}/pulls?per_page=100&page=2`);
      return [pull(43), pull(42), pull(91, { base: { repo: { full_name: 'other/repo' } } })];
    },
  );
  assert.equal(calls.length, 3);
  assert.deepEqual(change.pulls, [42, 43]);
  assert.equal(
    groupedChanges('example/ruphish', [change]),
    `### Fixes\n\n- resolve conflict \\([#42](https://github.com/example/ruphish/pull/42)\\), see #99 ([#43](https://github.com/example/ruphish/pull/43), [aaaaaaa](https://github.com/example/ruphish/commit/${commit})) by [@srothgan](https://github.com/srothgan)`,
  );
});

test('direct commits, bots and unlinked authors receive accurate credits and no invented PRs', async () => {
  const changes = ['1', '2', '3'].map((digit) => ({
    commit: digit.repeat(40),
    subject: 'chore(deps): update dependency',
  }));
  const details = [
    { author: { login: 'alice', html_url: 'https://github.com/alice' } },
    { author: { login: 'dependabot[bot]', html_url: 'https://github.com/apps/dependabot' } },
    { author: null, commit: { author: { name: 'Contributor *Name*' } } },
  ];
  const enriched = await enrichChanges(changes, 'example/ruphish', async (path) => {
    if (path.includes('/pulls?')) {
      return [];
    }
    const index = changes.findIndex((change) => path === `/commits/${change.commit}`);
    return { sha: changes[index].commit, ...details[index] };
  });
  const notes = groupedChanges('example/ruphish', enriched);
  assert.ok(notes.includes('by [@alice](https://github.com/alice)'));
  assert.ok(notes.includes('by [@dependabot\\[bot\\]](https://github.com/apps/dependabot)'));
  assert.ok(notes.includes('by Contributor \\*Name\\*'));
  assert.doesNotMatch(notes, /\/pull\/|\(deps\)|@Contributor/);
});

test('GitHub failures stop generation rather than silently losing PRs or attribution', async (t) => {
  const commit = 'a'.repeat(40);
  const changes = [{ commit, subject: 'feat: new feature' }];
  await assert.rejects(
    enrichChanges(changes, 'example/ruphish', async () => {
      throw new Error('API unavailable');
    }),
    /API unavailable/,
  );
  await assert.rejects(
    enrichChanges(changes, 'example/ruphish', async (path) =>
      path.includes('/pulls?') ? [] : { sha: 'b'.repeat(40) },
    ),
    /different commit/,
  );
  const fetch = t.mock.method(globalThis, 'fetch', async (url, options) => {
    assert.equal(url, `https://api.github.com/repos/example/ruphish/commits/${commit}`);
    assert.equal(options.headers.Authorization, 'Bearer fixture-token');
    return new Response('{}', { status: 403 });
  });
  await assert.rejects(
    github('example/ruphish', 'fixture-token')(`/commits/${commit}`),
    /GitHub GET .* HTTP 403/,
  );
  fetch.mock.restore();
});

function gitFixture(t, version = '1.0.0-dev.3') {
  const directory = temporary(t);
  const git = (...args) => run('git', args, { cwd: directory });
  git('init', '-b', 'main');
  git('config', 'user.name', 'Release fixture');
  git('config', 'user.email', 'fixture@example.test');
  git('config', 'commit.gpgsign', 'false');
  git('config', 'tag.gpgsign', 'false');
  git('config', 'core.autocrlf', 'false');
  git('config', 'core.hooksPath', join(directory, 'empty-hooks'));
  writeFileSync(join(directory, '.gitignore'), 'target/\n');
  writeFileSync(join(directory, 'Cargo.toml'), `[workspace.package]\nversion = "${version}"\n`);
  writeFileSync(join(directory, 'CHANGELOG.md'), '## Unreleased\nPlanned final changes.\n');
  mkdirSync(join(directory, 'target'));
  const preload = join(directory, 'target/mock-github.mjs');
  writeFileSync(
    preload,
    `
    globalThis.fetch = async (url) => {
      const match = /^https:\\/\\/api.github.com\\/repos\\/example\\/ruphish\\/commits\\/([a-f0-9]{40})(\\/pulls\\?per_page=100&page=1)?$/.exec(url);
      if (!match) throw new Error('Unexpected fixture API request: ' + url);
      const body = match[2] ? [] : {
        sha: match[1], author: { login: 'alice', html_url: 'https://github.com/alice' }
      };
      return new Response(JSON.stringify(body), { status: 200 });
    };
  `,
  );
  const commit = (subject) => {
    git('add', '--all');
    git('commit', '--allow-empty', '-m', subject);
    return git('rev-parse', 'HEAD');
  };
  const notes = () => readFileSync(join(directory, 'target/release-metadata/notes.md'), 'utf8');
  const invoke = (overrides = {}) =>
    spawnSync(
      process.execPath,
      ['--import', pathToFileURL(preload).href, resolve('.github/scripts/release.mjs'), 'metadata'],
      {
        cwd: directory,
        encoding: 'utf8',
        env: {
          ...process.env,
          GITHUB_EVENT_NAME: 'workflow_dispatch',
          GITHUB_SHA: git('rev-parse', 'HEAD'),
          GITHUB_REF: 'refs/heads/main',
          GITHUB_REPOSITORY: 'example/ruphish',
          GITHUB_OUTPUT: '',
          GH_TOKEN: 'fixture-token',
          ...overrides,
        },
      },
    );
  return { directory, git, commit, notes, invoke, preload };
}

test('final drafts span the previous final, share preview formatting and never change the repository', async (t) => {
  const repo = gitFixture(t);
  const previous = repo.commit('chore: previous final');
  repo.git('tag', 'v0.9.0');
  const feature = repo.commit('feat(ui): first preview feature');
  repo.git('tag', 'v1.0.0-dev.1');
  repo.git('tag', 'v1.0.0-dev.2');
  const selected = repo.commit('fix(api): selected correction');
  repo.commit('feat: later work');
  assert.deepEqual(selectBaseline('1.0.0', selected, { cwd: repo.directory }), {
    version: '0.9.0',
    commit: previous,
  });
  const before = {
    changelog: readFileSync(join(repo.directory, 'CHANGELOG.md'), 'utf8'),
    manifest: readFileSync(join(repo.directory, 'Cargo.toml'), 'utf8'),
    refs: repo.git('show-ref'),
    status: repo.git('status', '--porcelain'),
  };
  const result = spawnSync(
    process.execPath,
    [
      '--import',
      pathToFileURL(repo.preload).href,
      resolve('.github/scripts/release.mjs'),
      'notes',
      '1.0.0',
      'example/ruphish',
      selected,
    ],
    { cwd: repo.directory, encoding: 'utf8', env: { ...process.env, GH_TOKEN: 'fixture-token' } },
  );
  assert.equal(result.status, 0, result.stderr);
  assert.equal(
    result.stdout,
    `### Features\n\n- first preview feature ([${feature.slice(0, 7)}](https://github.com/example/ruphish/commit/${feature})) by [@alice](https://github.com/alice)\n\n### Fixes\n\n- selected correction ([${selected.slice(0, 7)}](https://github.com/example/ruphish/commit/${selected})) by [@alice](https://github.com/alice)\n`,
  );
  const api = async (path) =>
    path.includes('/pulls?')
      ? []
      : {
          sha: path.split('/').at(-1),
          author: { login: 'alice', html_url: 'https://github.com/alice' },
        };
  const dev = await generateNotes('1.0.0-dev.3', 'example/ruphish', selected, api, {
    cwd: repo.directory,
  });
  assert.ok(dev.startsWith('> [!WARNING]'));
  assert.ok(dev.endsWith(result.stdout.split('### Fixes')[1]));
  assert.ok(!dev.includes('first preview feature'));
  assert.doesNotMatch(
    result.stdout,
    /WARNING|\/compare\/|later work|previous final|\(ui\)|\(api\)/,
  );
  assert.equal(readFileSync(join(repo.directory, 'CHANGELOG.md'), 'utf8'), before.changelog);
  assert.equal(readFileSync(join(repo.directory, 'Cargo.toml'), 'utf8'), before.manifest);
  assert.equal(repo.git('show-ref'), before.refs);
  assert.equal(repo.git('status', '--porcelain'), before.status);
  assert.ok(!existsSync(join(repo.directory, 'target/release-metadata')));
});

test('final metadata publishes the titled changelog entry exactly without GitHub enrichment', (t) => {
  const repo = gitFixture(t, '1.0.0');
  const body =
    '- Handwritten [#42](https://github.com/example/ruphish/pull/42) by @alice.\n\nCustom upgrade instructions.';
  writeFileSync(
    join(repo.directory, 'CHANGELOG.md'),
    `## [1.0.0] - 2026-11-10 [Changes][v0.16.2]\n\n${body}\n\n## Unreleased\nFuture work.\n`,
  );
  repo.commit('chore: prepare final release');
  writeFileSync(
    repo.preload,
    'globalThis.fetch = () => { throw new Error("Final extraction must not call GitHub"); };',
  );
  const result = repo.invoke();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(repo.notes(), `${body}\n`);
  assert.equal(
    readJson(join(repo.directory, 'target/release-metadata/release.json')).publish,
    false,
  );
});

test('metadata and draft CLI surface API errors without writing partial output', (t) => {
  const repo = gitFixture(t);
  repo.commit('feat: fixture feature');
  writeFileSync(
    repo.preload,
    'globalThis.fetch = async () => new Response("{}", { status: 503 });',
  );
  const metadata = repo.invoke();
  assert.notEqual(metadata.status, 0);
  assert.match(metadata.stderr, /GitHub GET .* HTTP 503/);
  assert.ok(!existsSync(join(repo.directory, 'target/release-metadata')));
  const draft = spawnSync(
    process.execPath,
    [
      '--import',
      pathToFileURL(repo.preload).href,
      resolve('.github/scripts/release.mjs'),
      'notes',
      '1.0.0',
      'example/ruphish',
    ],
    { cwd: repo.directory, encoding: 'utf8', env: { ...process.env, GH_TOKEN: 'fixture-token' } },
  );
  assert.notEqual(draft.status, 0);
  assert.match(draft.stderr, /GitHub GET .* HTTP 503/);
  assert.equal(draft.stdout, '');
  assert.equal(repo.git('status', '--porcelain'), '');
});

test('the preceding reachable preview wins and history ends at the selected commit rather than HEAD', (t) => {
  const repo = gitFixture(t);
  repo.commit('chore: initialize');
  repo.git('tag', 'v0.9.0');
  repo.commit('feat: first preview');
  repo.git('tag', 'v1.0.0-dev.1');
  const previous = repo.commit('fix: previous preview');
  repo.git('tag', '-a', 'v1.0.0-dev.2', '-m', 'Previous preview');
  const selected = repo.commit('perf: selected preview');
  repo.git('tag', 'v1.0.0-dev.3');
  repo.commit('feat: later branch work');
  repo.git('tag', 'v1.0.0-dev.4');
  const baseline = selectBaseline('1.0.0-dev.3', selected, { cwd: repo.directory });
  assert.deepEqual(baseline, { version: '1.0.0-dev.2', commit: previous });
  assert.deepEqual(readHistory(selected, baseline, { cwd: repo.directory }), [
    { commit: selected, subject: 'perf: selected preview' },
  ]);
  assert.deepEqual(
    readHistory(selected, baseline, { cwd: repo.directory }),
    readHistory(selected, baseline, { cwd: repo.directory }),
  );
});

test('unreachable, unrelated, current, invalid and newer tags cannot replace the final fallback', (t) => {
  const repo = gitFixture(t);
  const root = repo.commit('chore: initial release');
  repo.git('tag', 'v0.8.0');
  repo.git('tag', 'v0.9.0');
  for (const tag of [
    'v0.9.1-dev.99',
    'v1.0.0-dev.3',
    'v1.0.0-dev.4',
    'v1.0.0',
    'v2.0.0',
    'v1.0.0-beta.1',
    'v1.0.0-dev.02',
    'latest',
    'release-v1.0.0-dev.2',
  ]) {
    repo.git('tag', tag);
  }
  repo.git('checkout', '--orphan', 'unrelated');
  repo.commit('feat: unrelated history');
  repo.git('tag', 'v1.0.0-dev.2');
  repo.git('tag', 'v0.9.9');
  repo.git('checkout', 'main');
  const selected = repo.commit('fix: first preview after the final release');
  const baseline = selectBaseline('1.0.0-dev.3', selected, { cwd: repo.directory });
  assert.deepEqual(baseline, { version: '0.9.0', commit: root });
  assert.deepEqual(readHistory(selected, baseline, { cwd: repo.directory }), [
    { commit: selected, subject: 'fix: first preview after the final release' },
  ]);
});

test('initial previews include all reachable non-merge history through the selected commit', (t) => {
  const repo = gitFixture(t, '1.0.0-dev.1');
  const first = repo.commit('Initial repository');
  repo.git('tag', 'v0.9.0-dev.5');
  const selected = repo.commit('feat(ui): Übergrößen (#13)');
  repo.commit('fix: after the selected commit');
  const baseline = selectBaseline('1.0.0-dev.1', selected, { cwd: repo.directory });
  assert.equal(baseline, null);
  const changes = readHistory(selected, baseline, { cwd: repo.directory });
  assert.deepEqual(changes, [
    { commit: first, subject: 'Initial repository' },
    { commit: selected, subject: 'feat(ui): Übergrößen (#13)' },
  ]);
  const notes = releaseNotes('1.0.0-dev.1', '', 'example/ruphish', {
    commit: selected,
    baseline,
    changes,
  });
  assert.ok(notes.includes('Initial repository'));
  assert.ok(notes.includes('Übergrößen \\(#13\\)'));
  assert.ok(!notes.includes('Changes since'));
  assert.ok(!notes.includes('after the selected commit'));
});

test('history includes direct and squashed PR commits and reverts without duplicating structural merges', (t) => {
  const repo = gitFixture(t);
  const initial = repo.commit('chore: initial release');
  repo.git('tag', 'v1.0.0-dev.2');
  writeFileSync(join(repo.directory, 'direct'), 'direct change');
  const direct = repo.commit('fix: direct change');
  repo.git('checkout', '-b', 'feature');
  const feature = repo.commit('feat(api): merged feature (#40)');
  repo.git('checkout', 'main');
  const docs = repo.commit('docs: clarify the feature');
  repo.git('merge', '--no-ff', 'feature', '-m', 'Merge pull request #40 from feature');
  const structural = repo.git('rev-parse', 'HEAD');
  repo.git('checkout', '-b', 'squashed');
  writeFileSync(join(repo.directory, 'squashed'), 'squashed change');
  const unsquashed = repo.commit('internal development commit');
  repo.git('checkout', 'main');
  repo.git('merge', '--squash', 'squashed');
  const squash = repo.commit('feat(ui)!: squashed PR (#41)');
  repo.git('revert', '--no-edit', direct);
  const reverted = repo.git('rev-parse', 'HEAD');
  const baseline = selectBaseline('1.0.0-dev.3', reverted, { cwd: repo.directory });
  assert.deepEqual(baseline, { version: '1.0.0-dev.2', commit: initial });
  const changes = readHistory(reverted, baseline, { cwd: repo.directory });
  assert.deepEqual(
    changes.map((change) => change.commit).sort(),
    [direct, feature, docs, squash, reverted].sort(),
  );
  assert.ok(!changes.some((change) => [structural, unsquashed].includes(change.commit)));
  assert.equal(changes.filter((change) => change.subject.includes('#40')).length, 1);
  assert.ok(changes.some((change) => change.subject.startsWith('Revert "fix: direct change"')));
  const notes = preview(changes, baseline, reverted);
  assert.ok(notes.includes('#40'));
  assert.ok(notes.includes('#41'));
  assert.ok(notes.includes('Revert "fix: direct change"'));
  assert.ok(!notes.includes('Merge pull request'));
});

test('Git failures abort history generation visibly', (t) => {
  const directory = temporary(t);
  assert.throws(
    () => selectBaseline('1.0.0-dev.1', 'a'.repeat(40), { cwd: directory }),
    /git failed:/,
  );
  const repo = gitFixture(t);
  const commit = repo.commit('Initial repository');
  assert.throws(() => readHistory('missing-commit', null, { cwd: repo.directory }), /git failed:/);
  assert.throws(
    () => readHistory(commit, { commit: 'missing-baseline' }, { cwd: repo.directory }),
    /git failed:/,
  );
  writeFileSync(join(directory, 'Cargo.toml'), '[workspace.package]\nversion = "1.0.0-dev.1"\n');
  const result = spawnSync(process.execPath, [resolve('.github/scripts/release.mjs'), 'metadata'], {
    cwd: directory,
    encoding: 'utf8',
    env: { ...process.env, GITHUB_EVENT_NAME: 'workflow_dispatch' },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /git failed:/);
});

test('manual metadata generates linked notes without publishing before the prospective tag exists', (t) => {
  const repo = gitFixture(t, '1.0.0-dev.1');
  repo.commit('chore: initial final release');
  repo.git('tag', 'v0.9.0');
  const selected = repo.commit('feat: preview work (#52)');
  assert.equal(repo.git('tag', '--list', 'v1.0.0-dev.1'), '');
  const result = repo.invoke();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(
    readJson(join(repo.directory, 'target/release-metadata/release.json')).publish,
    false,
  );
  assert.ok(repo.notes().includes(`/commit/${selected}`));
  assert.ok(repo.notes().includes('by [@alice](https://github.com/alice)'));
  assert.ok(!repo.notes().includes('/compare/'));
  assert.ok(repo.notes().includes('preview work \\(#52\\)'));
  assert.ok(!repo.notes().includes('/blob/'));
});

test('metadata checks real Git tag identity, main ancestry and manual non-publication', (t) => {
  const repo = gitFixture(t, '0.1.0-dev.1');
  const { directory, git, commit: createCommit } = repo;
  createCommit('Fixture');
  const commit = git('rev-parse', 'HEAD');
  git('update-ref', 'refs/remotes/origin/main', commit);
  git('tag', '-a', 'v0.1.0-dev.1', '-m', 'Preview');
  const invoke = (overrides = {}) =>
    repo.invoke({
      GITHUB_EVENT_NAME: 'push',
      GITHUB_SHA: commit,
      GITHUB_REF: 'refs/tags/v0.1.0-dev.1',
      ...overrides,
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

function publication(version = '0.1.0-dev.1', notes = 'Release notes') {
  const manifest = {
    ...parseVersion(version),
    tag: `v${version}`,
    commit: 'abc',
    publish: true,
    image: 'ghcr.io/example/ruphish',
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
    execute: () => publishRelease(manifest, notes, files, remote),
  };
}

test('publication retains the generated preview and authored final bodies unchanged', async () => {
  const final = releaseNotes(
    '1.0.0',
    '## 1.0.0 - 2026-11-10\n\n- Preserve [#42](https://github.com/example/ruphish/pull/42) by @alice.\n',
    'example/ruphish',
  );
  const development = preview([
    { commit: 'a'.repeat(40), subject: 'fix: preview correction (#43)' },
  ]);
  for (const [version, notes] of [
    ['1.0.0', final],
    ['1.0.0-dev.3', development],
  ]) {
    const p = publication(version, notes);
    await p.execute();
    assert.equal(p.state.release.body, notes);
  }
});

test('development releases publish all artifacts before release visibility and never advance latest', async () => {
  const p = publication('0.1.0-dev.1');
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

test('private GHCR packages retain the draft and resume without reuploading or repushing', async () => {
  const p = publication('0.1.0');
  const verify = p.remote.verifyPublicImage;
  const denied = new Error('unauthorized: authentication required');
  p.remote.verifyPublicImage = async () => {
    throw denied;
  };
  await assert.rejects(p.execute(), (error) => {
    assert.equal(error.cause, denied);
    assert.match(error.message, /ghcr.io\/example\/ruphish@sha256:verified/);
    assert.match(error.message, /Package settings and change visibility to Public/);
    assert.match(error.message, /rerun only the failed publication job/);
    return true;
  });
  assert.equal(p.state.release.draft, true);
  assert.equal(p.state.image, p.manifest.image_digest);
  assert.deepEqual(p.state.assets, p.files);
  assert(!p.state.events.includes('publish'));
  assert(!p.state.events.includes('latest'));

  p.remote.verifyPublicImage = verify;
  p.state.events = [];
  await p.execute();
  assert.equal(p.state.release.draft, false);
  assert.deepEqual(p.state.events, ['public-image', 'publish', 'latest']);
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
  const jobs = ['metadata', 'source', 'security', 'assets', 'native', 'image', 'bundle'];
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
