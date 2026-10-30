import assert from 'node:assert/strict';
import { readFileSync, readdirSync, writeFileSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { compareVersions, hash, parseVersion, readJson, run, writeJson } from './release.mjs';

export function seal(directory) {
  const assets = join(directory, 'assets');
  const files = readdirSync(assets)
    .filter((name) => !['SHA256SUMS', 'release-manifest.json'].includes(name))
    .sort();
  writeFileSync(
    join(assets, 'SHA256SUMS'),
    files.map((name) => `${hash(readFileSync(join(assets, name)))}  ${name}\n`).join(''),
  );
  const manifest = readJson(join(assets, 'release-manifest.json'));
  manifest.files = Object.fromEntries(
    [...files, 'SHA256SUMS'].map((name) => [name, hash(readFileSync(join(assets, name)))]),
  );
  writeJson(join(assets, 'release-manifest.json'), manifest);
}

// Pure orchestration: production adapters perform real HTTP/registry operations;
// tests inject failures at those boundaries to verify ordering and safe retries.
export async function publishRelease(manifest, notes, files, remote) {
  assert.equal(manifest.publish, true, 'Manual runs cannot publish');
  assert.equal(manifest.tag, `v${manifest.version}`);
  const version = parseVersion(manifest.version);
  assert.equal(manifest.channel, version.channel);
  assert.equal(
    await remote.tagCommit(manifest.tag),
    manifest.commit,
    'Tag moved after verification',
  );
  let release = await remote.findRelease(manifest.tag);
  if (release) {
    assert.equal(release.body, notes, 'Existing release notes differ');
    const saved = await remote.asset(release, 'release-manifest.json');
    assert(
      saved
        ? saved.equals(files['release-manifest.json'])
        : release.draft && release.assets?.length === 0,
      'Existing release belongs to different artifacts; do not overwrite',
    );
  }
  const current = await remote.imageDigest(manifest.version);
  assert(
    !current || current === manifest.image_digest,
    'Image version already exists with different content',
  );
  if (!release) {
    assert(!current, 'Published image without matching draft; inspect before retrying');
    release = await remote.createDraft(manifest, notes);
  }
  // Manifest first: subsequent retries must prove ownership before uploading anything.
  for (const name of [
    'release-manifest.json',
    ...Object.keys(files)
      .filter((name) => name !== 'release-manifest.json')
      .sort(),
  ]) {
    const existing = await remote.asset(release, name);
    if (existing) {
      assert(existing.equals(files[name]), `Existing asset differs: ${name}`);
    } else {
      assert(release.draft, `Published release is missing ${name}; never regenerate it`);
      await remote.upload(release, name, files[name]);
    }
  }
  if (!current) {
    await remote.pushImage(manifest.version);
  }
  assert.equal(
    await remote.imageDigest(manifest.version),
    manifest.image_digest,
    'Published image digest differs',
  );
  try {
    await remote.verifyPublicImage(manifest.version, manifest.image_digest);
  } catch (cause) {
    throw new Error(
      `Anonymous pull verification failed for ${manifest.image}@${manifest.image_digest}. ` +
        'Release publication and latest updates have stopped; uploaded artifacts are retained. ' +
        'New GHCR packages default to private. Open the package from the repository Packages sidebar, ' +
        'then Package settings and change visibility to Public. If already public, inspect the registry error below. ' +
        'After resolving it, rerun only the failed publication job in this workflow run; do not recreate the tag. ' +
        `Registry error: ${cause.message}`,
      { cause },
    );
  }
  const finals = (await remote.listReleases())
    .filter((r) => !r.draft && !r.prerelease)
    .map((r) => {
      const parsed = parseVersion(r.tag_name.replace(/^v/, ''));
      assert.equal(parsed.channel, 'final', 'Unexpected final release classification');
      return parsed.version;
    });
  const latest =
    version.channel === 'final' && finals.every((v) => compareVersions(v, manifest.version) <= 0);
  if (release.draft) {
    await remote.publish(release, version.channel !== 'final', latest);
  }
  if (latest) {
    await remote.advanceLatest(manifest.version, manifest.image_digest);
  }
}

export function github(repository, token) {
  assert(/^[\w.-]+\/[\w.-]+$/.test(repository));
  return async function request(
    path,
    { method = 'GET', body, bytes = false, missing = false } = {},
  ) {
    const response = await fetch(`https://api.github.com/repos/${repository}${path}`, {
      method,
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: bytes ? 'application/octet-stream' : 'application/vnd.github+json',
        'X-GitHub-Api-Version': '2022-11-28',
        ...(body ? { 'Content-Type': 'application/json' } : {}),
      },
      body: body ? JSON.stringify(body) : undefined,
      signal: AbortSignal.timeout(120_000),
    });
    if (missing && response.status === 404) {
      return null;
    }
    assert(response.ok, `GitHub ${method} ${path}: HTTP ${response.status}`);
    if (response.status === 204) {
      return null;
    }
    return bytes ? Buffer.from(await response.arrayBuffer()) : response.json();
  };
}

export async function listReleases(api) {
  const releases = [];
  for (let page = 1; ; page++) {
    const batch = await api(`/releases?per_page=100&page=${page}`);
    releases.push(...batch);
    if (batch.length < 100) {
      return releases;
    }
  }
}

async function adapters(manifest, directory) {
  const token = process.env.GH_TOKEN;
  assert(token, 'Publication token is required');
  const api = github(manifest.repository, token);
  const image = manifest.image.replace(/^ghcr.io\//, '');
  assert.equal(image, manifest.repository.toLowerCase());
  const imageManifest = async (tag) => {
    const login = await fetch(
      `https://ghcr.io/token?service=ghcr.io&scope=repository:${image}:pull,push`,
      {
        headers: {
          Authorization: `Basic ${Buffer.from(`${process.env.GITHUB_ACTOR}:${token}`).toString('base64')}`,
        },
        signal: AbortSignal.timeout(30_000),
      },
    );
    assert(login.ok, `Registry authentication: HTTP ${login.status}`);
    const authorization = `Bearer ${(await login.json()).token}`;
    const response = await fetch(`https://ghcr.io/v2/${image}/manifests/${tag}`, {
      headers: { Authorization: authorization, Accept: 'application/vnd.oci.image.index.v1+json' },
      signal: AbortSignal.timeout(30_000),
    });
    if (response.status === 404) {
      return null;
    }
    assert(response.ok, `Registry manifest ${tag}: HTTP ${response.status}`);
    const bytes = Buffer.from(await response.arrayBuffer());
    const digest = `sha256:${hash(bytes)}`;
    assert.equal(response.headers.get('docker-content-digest'), digest);
    return { digest, value: JSON.parse(bytes) };
  };
  const copyImage = (tag) =>
    run(
      'skopeo',
      [
        'copy',
        '--all',
        '--preserve-digests',
        `oci:${resolve(directory, 'image')}:release`,
        `docker://${manifest.image}:${tag}`,
      ],
      { stdio: ['ignore', 'inherit', 'pipe'] },
    );
  return {
    tagCommit: async (tag) => (await api(`/commits/${tag}`)).sha,
    // The by-tag endpoint only returns published releases. Listing includes drafts
    // for the publisher, allowing partial uploads to resume instead of duplicating them.
    findRelease: async (tag) => {
      const matches = (await listReleases(api)).filter((release) => release.tag_name === tag);
      assert(matches.length <= 1, 'Multiple releases use this tag; inspect before retrying');
      return matches[0] ?? null;
    },
    asset: async (release, name) => {
      const assets = await api(`/releases/${release.id}/assets?per_page=100`);
      release.assets = assets;
      const asset = assets.find((entry) => entry.name === name);
      // GitHub may leave a zero-length 'starter' asset after an interrupted upload.
      // Only remove that incomplete upload while the matching release is a draft.
      if (asset?.state === 'starter' && release.draft) {
        await api(`/releases/assets/${asset.id}`, { method: 'DELETE' });
        release.assets = assets.filter((entry) => entry.id !== asset.id);
        return null;
      }
      return asset ? api(`/releases/assets/${asset.id}`, { bytes: true }) : null;
    },
    createDraft: (meta, notes) =>
      api('/releases', {
        method: 'POST',
        body: {
          tag_name: meta.tag,
          target_commitish: meta.commit,
          name: `Ruphish ${meta.version}`,
          body: notes,
          draft: true,
          prerelease: meta.channel !== 'final',
          make_latest: 'false',
        },
      }),
    upload: async (release, name, bytes) => {
      const response = await fetch(
        `https://uploads.github.com/repos/${manifest.repository}/releases/${release.id}/assets?name=${encodeURIComponent(name)}`,
        {
          method: 'POST',
          headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/octet-stream' },
          body: bytes,
          signal: AbortSignal.timeout(300_000),
        },
      );
      assert(response.ok, `Asset upload ${name}: HTTP ${response.status}`);
    },
    imageDigest: async (tag) => (await imageManifest(tag))?.digest ?? null,
    pushImage: async (tag) => {
      copyImage(tag);
    },
    verifyPublicImage: async (_tag, digest) => {
      const pulled = join(process.env.RUNNER_TEMP, 'ruphish-anonymous-pull');
      run('skopeo', [
        'copy',
        '--src-no-creds',
        '--all',
        '--preserve-digests',
        `docker://${manifest.image}@${digest}`,
        `oci:${pulled}:release`,
      ]);
      const raw = run('skopeo', ['inspect', '--raw', `oci:${pulled}:release`]);
      assert.equal(
        `sha256:${hash(Buffer.from(raw))}`,
        digest,
        'Anonymous image digest differs from the verified release image',
      );
    },
    listReleases: () => listReleases(api),
    publish: (release, prerelease, latest) =>
      api(`/releases/${release.id}`, {
        method: 'PATCH',
        body: { draft: false, prerelease, make_latest: String(latest) },
      }),
    advanceLatest: async (version, digest) => {
      const old = await imageManifest('latest');
      if (old && old.digest !== digest) {
        const previous = old.value.annotations?.['org.opencontainers.image.version'];
        assert(previous, 'Existing latest image has no version; inspect before replacing');
        assert.equal(parseVersion(previous).channel, 'final');
        if (compareVersions(previous, version) > 0) {
          return;
        }
      }
      copyImage('latest');
      assert.equal((await imageManifest('latest'))?.digest, digest);
    },
  };
}

async function publish(directory) {
  assert.equal(process.env.GITHUB_EVENT_NAME, 'push', 'Only tag pushes may publish');
  const manifest = readJson(join(directory, 'assets/release-manifest.json'));
  assert.equal(process.env.GITHUB_REF, `refs/tags/${manifest.tag}`);
  assert.equal(process.env.GITHUB_REPOSITORY, manifest.repository);
  assert.equal(run('git', ['rev-parse', 'HEAD']), manifest.commit);
  const files = Object.fromEntries(
    readdirSync(join(directory, 'assets')).map((name) => [
      name,
      readFileSync(join(directory, 'assets', name)),
    ]),
  );
  assert.deepEqual(
    Object.keys(files).sort(),
    ['release-manifest.json', ...Object.keys(manifest.files)].sort(),
  );
  for (const [name, digest] of Object.entries(manifest.files)) {
    assert.equal(hash(files[name]), digest, `Artifact changed: ${name}`);
  }
  const raw = run('skopeo', ['inspect', '--raw', `oci:${resolve(directory, 'image')}:release`]);
  assert.equal(
    `sha256:${hash(Buffer.from(raw))}`,
    manifest.image_digest,
    'Retained image manifest changed',
  );
  const authfile = join(process.env.RUNNER_TEMP, 'ruphish-registry-auth.json');
  process.env.REGISTRY_AUTH_FILE = authfile;
  try {
    run(
      'skopeo',
      ['login', '--username', process.env.GITHUB_ACTOR, '--password-stdin', 'ghcr.io'],
      { input: process.env.GH_TOKEN },
    );
    await publishRelease(
      manifest,
      readFileSync(join(directory, 'notes.md'), 'utf8'),
      files,
      await adapters(manifest, directory),
    );
  } finally {
    rmSync(authfile, { force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [command, directory = 'target/release-bundle'] = process.argv.slice(2);
  if (command === 'seal') {
    seal(directory);
  } else if (command === 'publish') {
    await publish(directory);
  } else if (command === 'preflight') {
    const meta = readJson('target/release-metadata/release.json');
    if (meta.publish) {
      const release = await github(meta.repository, process.env.GH_TOKEN)(
        `/releases/tags/${meta.tag}`,
        { missing: true },
      );
      assert(
        !release || release.draft,
        'Version is already published; rerun only the original publication job to repair a partial publication',
      );
    }
  } else {
    throw new Error(`Unknown publication command: ${command}`);
  }
}
