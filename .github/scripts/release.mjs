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
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(dev|rc)\.([1-9]\d*))?$/.exec(
    version,
  );
  assert(match, `Unsupported release version: ${version}`);
  return {
    version,
    base: match.slice(1, 4).join('.'),
    channel: match[4] ?? 'final',
    number: match[5] ?? null,
  };
}

export function compareVersions(a, b) {
  const left = parseVersion(a);
  const right = parseVersion(b);
  const fields = (v) => [
    ...v.base.split('.').map(BigInt),
    BigInt({ dev: 0, rc: 1, final: 2 }[v.channel]),
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

export function releaseNotes(version, changelog, repository, previous) {
  const parsed = parseVersion(version);
  let notes;
  if (parsed.channel === 'final') {
    const sections = [
      ...changelog.replaceAll('\r\n', '\n').matchAll(/^## (.+)\n([\s\S]*?)(?=^## |$(?![\s\S]))/gm),
    ];
    const matches = sections.filter(
      (section) =>
        section[1].match(/^(?:\[)?([0-9]+\.[0-9]+\.[0-9]+)(?:\])? - \d{4}-\d{2}-\d{2}$/)?.[1] ===
        version,
    );
    assert.equal(matches.length, 1, `Expected one dated changelog entry for ${version}`);
    notes = matches[0][2].trim();
    assert(notes && !/\bTODO\b/.test(notes), 'Final release notes must be complete');
  } else {
    notes =
      parsed.channel === 'dev'
        ? `Development preview ${parsed.number} for Ruphish ${parsed.base}.\n\nFor testing and disposable installations. Upgrades between development previews or into final releases are not guaranteed.`
        : `Release candidate ${parsed.number} for Ruphish ${parsed.base}.\n\nFor final testing before release. Report release blockers before using this version in production.`;
    notes += `\n\n[Planned release changes](https://github.com/${repository}/blob/v${version}/CHANGELOG.md).`;
  }
  if (previous) {
    assert(compareVersions(previous, version) < 0, 'Comparison baseline must precede the release');
    notes += `\n\n[Changes since ${previous}](https://github.com/${repository}/compare/v${previous}...v${version}).`;
  }
  return `${notes}\n\nDocker: \`ghcr.io/${repository.toLowerCase()}:${version}\`.\n\nNative archives are unsigned; macOS archives are not notarized. Checksums and GitHub build provenance accompany the downloads.\n`;
}

export function metadata(env = process.env) {
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
  const tags = run('git', ['tag', '--merged', commit])
    .split('\n')
    .flatMap((tag) => {
      try {
        const v = parseVersion(tag.slice(1));
        return tag.startsWith('v') && compareVersions(v.version, version) < 0 ? [v] : [];
      } catch {
        return [];
      }
    });
  const previous = tags
    .filter((v) => (parsed.channel === 'final' ? v.channel === 'final' : v.base === parsed.base))
    .sort((a, b) => compareVersions(b.version, a.version))[0]?.version;
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
  writeFileSync(
    'target/release-metadata/notes.md',
    releaseNotes(version, readFileSync('CHANGELOG.md', 'utf8'), result.repository, previous),
  );
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
    metadata();
  } else if (command === 'package') {
    packageBinary(...args);
  } else if (command === 'assemble') {
    assemble();
  } else {
    throw new Error(`Unknown release command: ${command}`);
  }
}
