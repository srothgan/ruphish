// Disposable checks of the actual image defaults; no host ports or external network.
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { createHash, randomUUID } from 'node:crypto';
import { gunzipSync, brotliDecompressSync } from 'node:zlib';
import { establishContainerAccount, waitContainerReady } from './container.mjs';

const image = process.env.RUPHISH_APP_IMAGE ?? 'ruphish:local';

test('readiness observation rejects a container that exits during startup', async (t) => {
  const name = `ruphish-readiness-test-${randomUUID()}`;
  t.after(() => docker(['rm', '-f', name], true));
  docker([
    'run',
    '-d',
    '--name',
    name,
    '--network',
    'none',
    '-e',
    'ADMIN_SERVER_USE_TLS=invalid',
    image,
  ]);
  await assert.rejects(waitContainerReady(name), /exited before readiness/);
  assert.equal(
    docker(['inspect', '--format={{.State.ExitCode}}', name]).stdout.toString().trim(),
    '1',
  );
});

function docker(args, allowFailure = false) {
  const result = spawnSync('docker', args, { maxBuffer: 128 * 1024 * 1024 });
  assert.ifError(result.error);
  if (!allowFailure) {
    assert.equal(result.status, 0, result.stderr.toString());
  }
  return result;
}

test('invalid environment values and conflicting providers fail before startup without leaking secrets', () => {
  for (const environment of [
    ['ADMIN_SERVER_USE_TLS=private-invalid-value'],
    ['ADMIN_SERVER_TYPO=private-invalid-value'],
    [
      'DB_NAME=postgres',
      'POSTGRES_HOST=postgres',
      'POSTGRES_DATABASE=test',
      'POSTGRES_USERNAME=test',
      'POSTGRES_PASSWORD=private-invalid-value',
      'MYSQL_HOST=mysql',
      'MYSQL_DATABASE=test',
      'MYSQL_USERNAME=test',
      'MYSQL_PASSWORD=private-invalid-value',
    ],
  ]) {
    const result = docker(
      [
        'run',
        '--rm',
        '--network',
        'none',
        '--read-only',
        ...environment.flatMap((value) => ['-e', value]),
        image,
        '--disable-mailer',
      ],
      true,
    );
    assert.equal(result.status, 1);
    assert.match(result.stderr.toString(), /Invalid configuration/);
    assert(!result.stderr.includes('private-invalid-value'));
    assert(!result.stderr.includes('RUPHISH_READY'));
  }
});

test('image defaults serve embedded assets and preserve state with a read-only root filesystem', async (t) => {
  const name = `ruphish-image-test-${randomUUID()}`;
  const volume = `${name}-data`;
  const token = randomUUID();
  docker(['volume', 'create', volume]);
  t.after(() => {
    docker(['rm', '-f', name], true);
    docker(['volume', 'rm', volume]);
  });

  const start = (args, environment = []) =>
    docker([
      'run',
      '-d',
      '--name',
      name,
      '--network',
      'none',
      '--read-only',
      '--tmpfs',
      '/tmp:rw,noexec,nosuid,size=128m',
      '--mount',
      `type=volume,source=${volume},target=/var/lib/ruphish`,
      '-e',
      'GOPHISH_INITIAL_ADMIN_PASSWORD=synthetic-container-password',
      '-e',
      `GOPHISH_INITIAL_ADMIN_API_TOKEN=${token}`,
      ...environment.flatMap((value) => ['-e', value]),
      image,
      ...args,
    ])
      .stdout.toString()
      .trim();

  const request = (path, { port = 3333, headers = [], body } = {}) => {
    const args = [
      'exec',
      name,
      'curl',
      '--noproxy',
      '*',
      '--silent',
      '--show-error',
      '-D',
      '-',
      '-H',
      'Accept: application/json',
    ];
    for (const header of headers) {
      args.push('-H', header);
    }
    if (body !== undefined) {
      args.push('-H', 'Content-Type: application/json', '--data-binary', JSON.stringify(body));
    }
    args.push(`http://127.0.0.1:${port}${path}`);
    const response = docker(args).stdout;
    const split = response.indexOf('\r\n\r\n');
    assert(split > 0);
    const lines = response.subarray(0, split).toString().split('\r\n');
    return {
      status: Number(lines[0].split(' ')[1]),
      headers: new Map(
        lines.slice(1).map((line) => {
          const colon = line.indexOf(':');
          return [line.slice(0, colon).toLowerCase(), line.slice(colon + 1).trim()];
        }),
      ),
      body: response.subarray(split + 4),
    };
  };

  const first = start(['--disable-mailer']);
  await waitContainerReady(name);
  const inspect = JSON.parse(docker(['inspect', name]).stdout)[0];
  assert.equal(inspect.Config.User, '10001:10001');
  assert.equal(inspect.Path, '/opt/ruphish/ruphish');
  assert.equal(inspect.HostConfig.ReadonlyRootfs, true);
  assert.equal(request('/health/ready', { port: 8081 }).status, 200);
  const config = JSON.parse(docker(['exec', name, 'cat', '/etc/ruphish/config.json']).stdout);
  assert.deepEqual(
    config,
    JSON.parse(readFileSync(new URL('../../docker/config.json', import.meta.url))),
  );
  assert.equal(docker(['exec', name, 'test', '-e', '/opt/ruphish/dist'], true).status, 1);
  for (const tool of ['cargo', 'rustc', 'node', 'npm']) {
    assert.equal(
      docker(['exec', name, 'sh', '-c', 'command -v "$1"', 'sh', tool], true).status,
      127,
    );
  }
  for (const notice of ['LICENSE', 'THIRD-PARTY-NOTICES.md', 'THIRD-PARTY-NOTICES.txt']) {
    assert.equal(
      docker(['exec', name, 'test', '-s', `/usr/share/doc/ruphish/${notice}`]).status,
      0,
    );
  }
  const expectedBinary = createHash('sha256')
    .update(readFileSync(new URL('../../target/docker/ruphish', import.meta.url)))
    .digest('hex');
  assert.equal(
    docker(['exec', name, 'sha256sum', '/opt/ruphish/ruphish']).stdout.toString().split(' ')[0],
    expectedBinary,
  );
  const version = docker(['exec', name, '/opt/ruphish/ruphish', '--version'])
    .stderr.toString()
    .trim();
  const expectedVersion = readFileSync(new URL('../../Cargo.toml', import.meta.url), 'utf8').match(
    /\[workspace.package\]\s+version = "([^"]+)"/,
  )[1];
  assert.equal(version, `Ruphish ${expectedVersion}`);

  const manifest = request('/asset-manifest.json');
  assert.equal(manifest.status, 200);
  const expectedManifest = readFileSync(
    new URL('../../dist/admin/asset-manifest.json', import.meta.url),
  );
  assert.deepEqual(manifest.body, expectedManifest);
  const [wasm, digest] = Object.entries(JSON.parse(manifest.body)).find(([path]) =>
    path.endsWith('.wasm'),
  );
  for (const [encoding, decode] of [
    ['gzip', gunzipSync],
    ['br', brotliDecompressSync],
  ]) {
    const response = request(`/${wasm}`, {
      headers: [`Accept-Encoding: ${encoding}, identity;q=0`],
    });
    assert.equal(response.status, 200);
    assert.equal(response.headers.get('content-encoding'), encoding);
    assert.match(response.headers.get('vary'), /accept-encoding/i);
    assert.equal(createHash('sha256').update(decode(response.body)).digest('hex'), digest);
    const cached = request(`/${wasm}`, {
      headers: [`Accept-Encoding: ${encoding}`, `If-None-Match: ${response.headers.get('etag')}`],
    });
    assert.equal(cached.status, 304);
    assert.equal(cached.body.length, 0);
  }
  const login = request('/login');
  assert.equal(login.status, 200);
  assert.equal(login.headers.get('cache-control'), 'no-store');
  establishContainerAccount(name, 'synthetic-container-password');
  const auth = [`Authorization: Bearer ${token}`];
  const created = request('/api/groups/', {
    headers: auth,
    body: { name: 'Container persistence test', targets: [{ email: 'container@example.test' }] },
  });
  assert.equal(created.status, 201);
  const group = JSON.parse(created.body);

  docker(['stop', '--time', '30', name]);
  assert.equal(
    docker(['inspect', '--format={{.State.ExitCode}}', name]).stdout.toString().trim(),
    '0',
  );
  docker(['rm', name]);
  const second = start(
    ['--mode', 'admin', '--disable-mailer'],
    [
      'ADMIN_SERVER_LISTEN_URL=0.0.0.0:13333',
      'ADMIN_SERVER_USE_TLS=false',
      'LOGGING_LEVEL=warn',
      'RUPHISH_HEALTH_URL=http://127.0.0.1:13333/health/ready',
    ],
  );
  assert.notEqual(first, second);
  await waitContainerReady(name);
  const persisted = request(`/api/groups/${group.id}`, { headers: auth, port: 13333 });
  assert.equal(persisted.status, 200);
  assert.equal(JSON.parse(persisted.body).name, group.name);
  assert.deepEqual(
    JSON.parse(docker(['exec', name, 'cat', '/etc/ruphish/config.json']).stdout),
    config,
  );
  docker(['stop', '--time', '30', name]);
  assert.equal(
    docker(['inspect', '--format={{.State.ExitCode}}', name]).stdout.toString().trim(),
    '0',
  );
});
