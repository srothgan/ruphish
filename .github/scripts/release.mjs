import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import {
  appendFileSync,
  chmodSync,
  copyFileSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { github } from './github.mjs';

export const targets = [
  'x86_64-pc-windows-msvc',
  'aarch64-pc-windows-msvc',
  'x86_64-unknown-linux-gnu',
  'aarch64-unknown-linux-gnu',
  'aarch64-apple-darwin',
];
export const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
export const readJson = (path) => JSON.parse(readFileSync(path, 'utf8'));
export const writeJson = (path, value) =>
  writeFileSync(path, JSON.stringify(value, null, 2) + '\n');
export function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
    ...options,
  });
  assert.ifError(result.error);
  assert.equal(result.status, 0, `${command} failed: ${result.stderr}`);
  return (result.stdout ?? '').trim();
}

export function parseVersion(version) {
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-dev\.([1-9]\d*))?$/.exec(version);
  assert(match, `Unsupported release version: ${version}`);
  return {
    version,
    base: match.slice(1, 4).join('.'),
    channel: match[4] ? 'dev' : 'final',
    number: match[4] ?? null,
  };
}

export function compareVersions(a, b) {
  const left = parseVersion(a);
  const right = parseVersion(b);
  const fields = (v) => [
    ...v.base.split('.').map(BigInt),
    BigInt(v.channel === 'final'),
    BigInt(v.number ?? 0),
  ];
  const x = fields(left);
  const y = fields(right);
  for (let i = 0; i < x.length; i++) {
    if (x[i] !== y[i]) {
      return x[i] > y[i] ? 1 : -1;
    }
  }
  return 0;
}

const changeGroups = new Map([
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
]);

export function selectBaseline(version, commit, options = {}) {
  const current = parseVersion(version);
  const candidates = run('git', ['tag', '--merged', commit], options)
    .split(/\r?\n/)
    .flatMap((tag) => {
      if (!tag.startsWith('v')) {
        return [];
      }
      try {
        return [parseVersion(tag.slice(1))];
      } catch {
        return [];
      }
    })
    .filter((tag) => compareVersions(tag.version, version) < 0)
    .sort((a, b) => compareVersions(b.version, a.version));
  const previous =
    candidates.find(
      (tag) => current.channel === 'dev' && tag.channel === 'dev' && tag.base === current.base,
    ) ?? candidates.find((tag) => tag.channel === 'final');
  return previous
    ? {
        version: previous.version,
        commit: run('git', ['rev-parse', `refs/tags/v${previous.version}^{commit}`], options),
      }
    : null;
}

export function readHistory(commit, baseline, options = {}) {
  const output = run(
    'git',
    [
      'log',
      '--no-merges',
      '--reverse',
      '--topo-order',
      '--encoding=UTF-8',
      '-z',
      '--format=%H%x00%s',
      baseline ? `${baseline.commit}..${commit}` : commit,
      '--',
    ],
    options,
  );
  if (!output) {
    return [];
  }
  const fields = output.split('\0');
  assert.equal(fields.pop(), '', 'Git history must end with a record delimiter');
  assert.equal(fields.length % 2, 0, 'Git history must contain commit/subject pairs');
  const changes = [];
  for (let i = 0; i < fields.length; i += 2) {
    assert(/^[a-f0-9]{40}$/.test(fields[i]), 'Invalid Git history commit');
    changes.push({ commit: fields[i], subject: fields[i + 1] });
  }
  return changes;
}

export function parseSubject(subject) {
  const match = /^([a-z]+)(?:\(([^()]+)\))?(!)?:\s+(.+)$/.exec(subject);
  return match
    ? {
        type: match[1],
        scope: match[2] ?? null,
        breaking: Boolean(match[3]),
        description: match[4],
      }
    : { type: null, scope: null, breaking: false, description: subject };
}

function escapeMarkdown(text) {
  // Keep authored #123 references available to GitHub's issue/PR autolinker.
  return text.replaceAll('&', '&amp;').replace(/[\\`*_{}[\]()<>!|~+.-]|#(?!\d)/g, '\\$&');
}

export async function enrichChanges(changes, repository, api) {
  const enriched = [];
  for (const change of changes) {
    const [details, firstPage] = await Promise.all([
      api(`/commits/${change.commit}`),
      api(`/commits/${change.commit}/pulls?per_page=100&page=1`),
    ]);
    assert.equal(details.sha, change.commit, 'GitHub returned a different commit');
    const pulls = new Set();
    let batch = firstPage;
    for (let page = 1; ; page++) {
      assert(Array.isArray(batch), 'Expected GitHub pull request list');
      for (const pull of batch) {
        if (pull.merged_at && pull.base?.repo?.full_name === repository) {
          assert(Number.isSafeInteger(pull.number) && pull.number > 0, 'Invalid PR number');
          pulls.add(pull.number);
        }
      }
      if (batch.length < 100) {
        break;
      }
      batch = await api(`/commits/${change.commit}/pulls?per_page=100&page=${page + 1}`);
    }
    const author = details.author?.login
      ? { login: details.author.login, url: details.author.html_url }
      : { name: details.commit?.author?.name };
    enriched.push({ ...change, pulls: [...pulls.values()].sort((a, b) => a - b), author });
  }
  return enriched;
}

function linkedDescription(text, repository, pulls) {
  let rendered = '';
  let offset = 0;
  for (const match of text.matchAll(/(?<![\w/#])#([1-9]\d*)\b/g)) {
    rendered += escapeMarkdown(text.slice(offset, match.index));
    const number = Number(match[1]);
    rendered += pulls.includes(number)
      ? `[#${number}](https://github.com/${repository}/pull/${number})`
      : match[0];
    offset = match.index + match[0].length;
  }
  return rendered + escapeMarkdown(text.slice(offset));
}

export function groupedChanges(repository, changes) {
  const groups = new Map([...changeGroups.values(), 'Other changes'].map((name) => [name, []]));
  for (const change of changes) {
    const subject = parseSubject(change.subject);
    const group = changeGroups.get(subject.type);
    const text = group ? subject.description : change.subject;
    const breaking = group && subject.breaking ? '**Breaking change:** ' : '';
    const pulls = change.pulls ?? [];
    const references = new Set(
      [...text.matchAll(/(?<![\w/#])#([1-9]\d*)\b/g)].map((m) => Number(m[1])),
    );
    const links = [
      ...pulls
        .filter((number) => !references.has(number))
        .map((number) => `[#${number}](https://github.com/${repository}/pull/${number})`),
      `[${change.commit.slice(0, 7)}](https://github.com/${repository}/commit/${change.commit})`,
    ];
    let credit = '';
    if (change.author?.login) {
      assert(
        /^https:\/\/github\.com\/[\w[\]/.-]+$/.test(change.author.url),
        'Invalid contributor URL',
      );
      credit = ` by [@${escapeMarkdown(change.author.login)}](${change.author.url})`;
    } else if (change.author?.name) {
      credit = ` by ${escapeMarkdown(change.author.name)}`;
    }
    groups
      .get(group ?? 'Other changes')
      .push(
        `- ${breaking}${linkedDescription(text || '(no subject)', repository, pulls)} (${links.join(', ')})${credit}`,
      );
  }
  return [...groups]
    .filter(([, entries]) => entries.length)
    .map(([name, entries]) => `### ${name}\n\n${entries.join('\n')}`)
    .join('\n\n');
}

function developmentNotes(parsed, repository, { changes }) {
  return [
    '> [!WARNING]',
    `> **Development preview ${parsed.number} for Ruphish ${parsed.base} - testing only.**`,
    '> Use a disposable installation. Upgrades between previews or to the final release are not guaranteed; a fresh installation may be required.',
    '',
    groupedChanges(repository, changes),
    '',
  ].join('\n');
}

function notesApi(repository, env) {
  return github(repository, env.GH_TOKEN || env.GITHUB_TOKEN || run('gh', ['auth', 'token']));
}

export async function generateNotes(version, repository, ref = 'HEAD', api, options = {}) {
  const parsed = parseVersion(version);
  const commit = run(
    'git',
    ['rev-parse', '--verify', '--end-of-options', `${ref}^{commit}`],
    options,
  );
  assert(/^[a-f0-9]{40}$/.test(commit), 'Invalid release commit');
  const baseline = selectBaseline(version, commit, options);
  const changes = await enrichChanges(
    readHistory(commit, baseline, options),
    repository,
    api ?? notesApi(repository, process.env),
  );
  return parsed.channel === 'dev'
    ? developmentNotes(parsed, repository, { changes })
    : `${groupedChanges(repository, changes)}\n`;
}

export function releaseNotes(version, changelog, repository, history) {
  const parsed = parseVersion(version);
  if (parsed.channel === 'final') {
    const sections = [
      ...changelog.replaceAll('\r\n', '\n').matchAll(/^## (.+)\n([\s\S]*?)(?=^## |$(?![\s\S]))/gm),
    ];
    const matches = sections.filter(
      (section) =>
        section[1].match(
          /^(?:\[)?([0-9]+\.[0-9]+\.[0-9]+)(?:\])? - \d{4}-\d{2}-\d{2}(?:[ \t]+.+)?$/,
        )?.[1] === version,
    );
    assert.equal(matches.length, 1, `Expected one dated changelog entry for ${version}`);
    const notes = matches[0][2].trim();
    assert(notes && !/\bTODO\b/.test(notes), 'Final release notes must be complete');
    return `${notes}\n`;
  }
  return developmentNotes(parsed, repository, history);
}

export async function metadata(env = process.env, api) {
  const manifest = readFileSync('Cargo.toml', 'utf8');
  const version = manifest.match(/\[workspace.package\]\s+version = "([^"]+)"/)[1];
  const parsed = parseVersion(version);
  const commit = run('git', ['rev-parse', 'HEAD']);
  assert(/^[a-f0-9]{40}$/.test(commit));
  assert(
    ['push', 'workflow_dispatch'].includes(env.GITHUB_EVENT_NAME),
    'Unsupported release event',
  );
  assert.equal(
    commit,
    run('git', ['rev-parse', `${env.GITHUB_SHA}^{commit}`]),
    'Checkout must match the selected source',
  );
  const publish = env.GITHUB_EVENT_NAME === 'push';
  if (publish) {
    assert.equal(env.GITHUB_REF, `refs/tags/v${version}`, 'Tag and Cargo version must agree');
    assert.equal(
      run('git', ['cat-file', '-t', env.GITHUB_REF]),
      'tag',
      'Use an annotated release tag',
    );
    run('git', ['merge-base', '--is-ancestor', commit, 'refs/remotes/origin/main']);
  }
  assert(/^[\w.-]+\/[\w.-]+$/.test(env.GITHUB_REPOSITORY), 'Invalid repository');
  const notes =
    parsed.channel === 'dev'
      ? await generateNotes(
          version,
          env.GITHUB_REPOSITORY,
          commit,
          api ?? notesApi(env.GITHUB_REPOSITORY, env),
        )
      : releaseNotes(version, readFileSync('CHANGELOG.md', 'utf8'), env.GITHUB_REPOSITORY);
  const result = {
    ...parsed,
    commit,
    tag: `v${version}`,
    repository: env.GITHUB_REPOSITORY,
    image: `ghcr.io/${env.GITHUB_REPOSITORY.toLowerCase()}`,
    publish,
  };
  mkdirSync('target/release-metadata', { recursive: true });
  writeJson('target/release-metadata/release.json', result);
  writeFileSync('target/release-metadata/notes.md', notes);
  if (env.GITHUB_OUTPUT) {
    for (const [key, value] of Object.entries(result)) {
      appendFileSync(env.GITHUB_OUTPUT, `${key}=${value}\n`);
    }
  }
  return result;
}

export function packageBinary(target, binary) {
  assert(targets.includes(target), 'Unsupported native target');
  const meta = readJson('target/release-metadata/release.json');
  const name = `ruphish-${meta.version}-${target}`;
  const root = mkdtempSync(join(tmpdir(), 'ruphish-package-'));
  const directory = join(root, name);
  mkdirSync(directory);
  const executable = target.includes('windows') ? 'ruphish.exe' : 'ruphish';
  copyFileSync(binary, join(directory, executable));
  chmodSync(join(directory, executable), 0o755);
  for (const file of ['LICENSE', 'THIRD-PARTY-NOTICES.md']) {
    copyFileSync(file, join(directory, file));
  }
  copyFileSync('dist/admin/THIRD-PARTY-NOTICES.txt', join(directory, 'THIRD-PARTY-NOTICES.txt'));
  writeJson(join(directory, 'build.json'), {
    version: meta.version,
    commit: meta.commit,
    target,
    rustc: run('rustc', ['--version']),
    executable_sha256: hash(readFileSync(binary)),
    assets_sha256: hash(readFileSync('dist/admin/asset-manifest.json')),
  });
  const extension = target.includes('windows') ? 'zip' : 'tar.gz';
  mkdirSync('target/release-packages', { recursive: true });
  const archive = resolve('target/release-packages', `${name}.${extension}`);
  const tar =
    process.platform === 'win32' ? join(process.env.SystemRoot, 'System32', 'tar.exe') : 'tar';
  run(
    tar,
    target.includes('windows')
      ? ['-a', '-cf', archive, '-C', root, name]
      : ['-czf', archive, '-C', root, name],
  );
  const extracted = mkdtempSync(join(tmpdir(), 'ruphish-extracted-'));
  run(tar, ['-xf', archive, '-C', extracted]);
  const unpacked = join(extracted, name);
  assert.deepEqual(readdirSync(unpacked).sort(), readdirSync(directory).sort());
  for (const file of readdirSync(directory)) {
    assert.equal(
      hash(readFileSync(join(unpacked, file))),
      hash(readFileSync(join(directory, file))),
      `Archive changed ${file}`,
    );
  }
  if (process.env.GITHUB_ENV) {
    appendFileSync(process.env.GITHUB_ENV, `RUPHISH_TEST_BINARY=${join(unpacked, executable)}\n`);
  }
  return archive;
}

// Join the verified architecture manifests without rebuilding or recompressing any image.
export function combineImages(inputs, output) {
  mkdirSync(join(output, 'blobs/sha256'), { recursive: true });
  const manifests = [];
  for (const input of inputs) {
    const index = readJson(join(input.directory, 'index.json'));
    assert.equal(index.manifests.length, 1, 'Expected one image per architecture');
    const descriptor = index.manifests[0];
    const blob = (entry) => {
      assert(/^sha256:[a-f0-9]{64}$/.test(entry.digest));
      const bytes = readFileSync(join(input.directory, 'blobs/sha256', entry.digest.slice(7)));
      assert.equal(hash(bytes), entry.digest.slice(7), 'OCI digest mismatch');
      assert.equal(bytes.length, entry.size, 'OCI size mismatch');
      copyFileSync(
        join(input.directory, 'blobs/sha256', entry.digest.slice(7)),
        join(output, 'blobs/sha256', entry.digest.slice(7)),
      );
      return bytes;
    };
    const manifest = JSON.parse(blob(descriptor));
    assert.equal(manifest.mediaType, 'application/vnd.oci.image.manifest.v1+json');
    const config = JSON.parse(blob(manifest.config));
    assert.equal(config.os, 'linux');
    assert.equal(config.architecture, input.architecture);
    assert.equal(config.config.Labels['org.opencontainers.image.version'], input.version);
    assert.equal(config.config.Labels['org.opencontainers.image.revision'], input.commit);
    for (const layer of manifest.layers) {
      blob(layer);
    }
    manifests.push({
      mediaType: descriptor.mediaType,
      digest: descriptor.digest,
      size: descriptor.size,
      platform: { os: 'linux', architecture: input.architecture },
    });
  }
  assert.deepEqual(manifests.map((m) => m.platform.architecture).sort(), ['amd64', 'arm64']);
  const content = Buffer.from(
    JSON.stringify({
      schemaVersion: 2,
      mediaType: 'application/vnd.oci.image.index.v1+json',
      manifests,
      annotations: {
        'org.opencontainers.image.version': inputs[0].version,
        'org.opencontainers.image.revision': inputs[0].commit,
      },
    }),
  );
  const digest = `sha256:${hash(content)}`;
  writeFileSync(join(output, 'blobs/sha256', digest.slice(7)), content);
  writeJson(join(output, 'oci-layout'), { imageLayoutVersion: '1.0.0' });
  writeJson(join(output, 'index.json'), {
    schemaVersion: 2,
    manifests: [
      {
        mediaType: 'application/vnd.oci.image.index.v1+json',
        digest,
        size: content.length,
        annotations: { 'org.opencontainers.image.ref.name': 'release' },
      },
    ],
  });
  return digest;
}

export function assemble() {
  const meta = readJson('target/release-metadata/release.json');
  const destination = 'target/release-bundle';
  const assets = join(destination, 'assets');
  mkdirSync(assets, { recursive: true });
  for (const target of targets) {
    const name = `ruphish-${meta.version}-${target}.${target.includes('windows') ? 'zip' : 'tar.gz'}`;
    copyFileSync(join('target/release-inputs', `native-${target}`, name), join(assets, name));
  }
  const inputs = ['amd64', 'arm64'].map((architecture) => {
    const directory = mkdtempSync(join(tmpdir(), 'ruphish-oci-'));
    run('tar', ['-xf', `target/release-inputs/image-${architecture}/image.tar`, '-C', directory]);
    return { directory, architecture, ...meta };
  });
  const digest = combineImages(inputs, join(destination, 'image'));
  writeJson(join(assets, 'release-manifest.json'), {
    ...meta,
    image_digest: digest,
  });
  cpSync('target/release-metadata/notes.md', join(destination, 'notes.md'));
  if (process.env.GITHUB_OUTPUT) {
    appendFileSync(process.env.GITHUB_OUTPUT, `digest=${digest}\n`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [command, ...args] = process.argv.slice(2);
  if (command === 'metadata') {
    await metadata();
  } else if (command === 'notes') {
    assert(
      args.length >= 2 && args.length <= 3,
      'Usage: release.mjs notes VERSION OWNER/REPO [COMMIT]',
    );
    process.stdout.write(await generateNotes(...args));
  } else if (command === 'package') {
    packageBinary(...args);
  } else if (command === 'assemble') {
    assemble();
  } else {
    throw new Error(`Unknown release command: ${command}`);
  }
}
