// Protocol acceptance against the real Docker app, DNS, TLS and SMTP services.
// Browser enforcement remains a separate manual acceptance gate.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { request } from 'node:https';
import { randomUUID } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { resolve, sep } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

const data = process.env.RUPHISH_ACCEPTANCE_DATA_DIRECTORY
  ? pathToFileURL(resolve(process.env.RUPHISH_ACCEPTANCE_DATA_DIRECTORY) + sep)
  : new URL('./data/', import.meta.url);
const env = Object.fromEntries(
  (await readFile(new URL('bootstrap.env', data), 'utf8'))
    .trim()
    .split('\n')
    .map((line) => line.split('=')),
);
const ca = await readFile(new URL('ca.pem', data));
const admin = process.env.RUPHISH_ACCEPTANCE_ADMIN ?? 'http://127.0.0.1:3334';
const inboxBase = process.env.RUPHISH_ACCEPTANCE_INBOX ?? 'http://127.0.0.1:8026';
const publicIp = process.env.RUPHISH_ACCEPTANCE_PUBLIC_IP ?? '127.0.0.1';
const origin = 'https://good.ruphish.test:8443';
async function api(path, method = 'GET', body) {
  const response = await fetch(`${admin}${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${env.GOPHISH_INITIAL_ADMIN_API_TOKEN}`,
      'Content-Type': 'application/json',
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  assert.equal(response.ok, true, `${method} ${path}: ${response.status}`);
  return response.json();
}
function publicRequest(
  path,
  { body, host = 'good.ruphish.test:8443', contentType = 'application/json' } = {},
) {
  return new Promise((resolve, reject) => {
    // Pin only resolution; ordinary certificate-chain and hostname checks remain enabled.
    const req = request(
      `${origin}${path}`,
      {
        // Authenticate the real TLS peer even when a test spoofs HTTP Host.
        ca,
        servername: new URL(origin).hostname,
        lookup: (_host, options, callback) =>
          options.all
            ? callback(null, [{ address: publicIp, family: 4 }])
            : callback(null, publicIp, 4),
        method: body === undefined ? 'GET' : 'POST',
        agent: false,
        headers: { Host: host, 'Content-Type': contentType },
      },
      (response) => {
        const chunks = [];
        response.on('data', (chunk) => chunks.push(chunk));
        response.on('end', () =>
          resolve({
            status: response.statusCode,
            headers: response.headers,
            body: Buffer.concat(chunks).toString('utf8'),
          }),
        );
        response.on('error', reject);
      },
    );
    req.on('error', reject);
    req.end(body);
  });
}
async function until(observe, predicate) {
  while (true) {
    const value = await observe();
    if (predicate(value)) {
      return value;
    }
    await delay(100);
  }
}

function docker(args) {
  const result = spawnSync('docker', args, { encoding: 'utf8' });
  assert.ifError(result.error);
  return result;
}
const compose = ['compose', '-f', fileURLToPath(new URL('./compose.yaml', import.meta.url))];

async function resourceIdentities() {
  const identities = {};
  for (const resource of ['users', 'groups', 'templates', 'pages', 'smtp', 'campaigns']) {
    identities[resource] = (await api(`/api/${resource}/`))
      .map((item) => item.id)
      .sort((a, b) => a - b);
  }
  return identities;
}

test('runtime image exposes health on HTTP and verified HTTPS and rejects failed probes', async () => {
  for (const [path, status] of [
    ['/health/live', 'live'],
    ['/health/ready', 'ready'],
  ]) {
    const response = await fetch(`${admin}${path}`, {});
    assert.equal(response.status, 200);
    assert.equal(response.headers.get('cache-control'), 'no-store');
    assert.deepEqual(response.headers.getSetCookie(), []);
    assert.deepEqual(await response.json(), { status });
    const secure = await publicRequest(path);
    assert.equal(secure.status, 200);
    assert.equal(secure.headers['cache-control'], 'no-store');
    assert.equal(secure.headers['set-cookie'], undefined);
    assert.deepEqual(JSON.parse(secure.body), { status });
  }
  const identity = docker([...compose, 'exec', '-T', 'app', 'id', '-u']);
  assert.equal(identity.status, 0, identity.stderr);
  assert.equal(identity.stdout.trim(), '10001');
  const passing = docker([
    ...compose,
    'exec',
    '-T',
    '-e',
    'http_proxy=http://127.0.0.1:1',
    'app',
    'ruphish-healthcheck',
  ]);
  assert.equal(passing.status, 0, passing.stderr);
  const container = docker([...compose, 'ps', '-q', 'app']);
  assert.equal(container.status, 0, container.stderr);
  const image = docker(['inspect', '--format={{.Image}}', container.stdout.trim()]);
  assert.equal(image.status, 0, image.stderr);
  // A disposable probe client uses this same image and fixture, with no new service or port.
  const secureProbe = docker([
    'run',
    '--rm',
    '--network',
    'ruphish-acceptance_lab',
    '--add-host',
    'good.ruphish.test:172.30.53.10',
    '--volumes-from',
    `${container.stdout.trim()}:ro`,
    '-e',
    'RUPHISH_HEALTH_URL=https://good.ruphish.test:8443/health/ready',
    '-e',
    'RUPHISH_HEALTH_CA=/lab/data/ca.pem',
    '--entrypoint',
    '/usr/local/bin/ruphish-healthcheck',
    image.stdout.trim(),
  ]);
  assert.equal(secureProbe.status, 0, secureProbe.stderr);
  for (const url of [
    'http://127.0.0.1:3333/', // Login redirect must not count as readiness.
    'http://127.0.0.1:3333/api/users/', // Authentication failure.
    'http://127.0.0.1:1/health/ready', // Connection refused.
    'https://127.0.0.1:8443/health/ready', // Untrusted CA and wrong hostname.
  ]) {
    const rejected = docker([
      ...compose,
      'exec',
      '-T',
      '-e',
      `RUPHISH_HEALTH_URL=${url}`,
      'app',
      'ruphish-healthcheck',
    ]);
    assert.equal(rejected.status, 1, `Probe must reject ${url}: ${rejected.stderr}`);
    assert.equal(rejected.stdout, '');
    assert.equal(rejected.stderr.trim(), 'Ruphish readiness probe failed');
  }
});

test('verified domain -> SMTP inbox -> public events -> completion, without participant values', async () => {
  // The user may repair the demonstration's bad domain. Keep rejection coverage
  // independent of that mutable example; lab.mjs demo owns wrong-TXT acceptance.
  const unregistered = `unregistered-${randomUUID()}.ruphish.test`;
  const domains = await api('/api/domains/');
  assert.equal(domains.find((item) => item.hostname === 'good.ruphish.test')?.eligible, true);
  assert(!domains.some((item) => item.hostname === unregistered));
  const eligible = await api('/api/domains/validated/');
  assert(eligible.some((item) => item.origin === origin));
  assert(!eligible.some((item) => item.hostname === unregistered));
  const profiles = await api('/api/smtp/');
  assert.equal(
    profiles.find((item) => item.name === 'Local inbox only')?.host,
    '172.30.53.20:1025',
    'Fixture SMTP must still target the local inbox',
  );

  const run = randomUUID();
  const email = `protocol-${run}@example.test`;
  const group = await api('/api/groups/', 'POST', {
    name: `Protocol ${run}`,
    targets: [{ email, first_name: 'Protocol' }],
  });
  const template = (await api('/api/templates/')).find(
    (item) => item.name === 'Local privacy email',
  );
  const page = (await api('/api/pages/')).find((item) => item.name === 'Local privacy page');
  const profile = profiles.find((item) => item.name === 'Local inbox only');
  const campaign = await api('/api/campaigns/', 'POST', {
    name: `Protocol ${run}`,
    url: origin,
    selection: {
      groups: [{ id: group.id }],
      template: { id: template.id },
      page: { id: page.id },
      smtp: { id: profile.id },
    },
  });
  const campaignPath = `/api/campaigns/${campaign.id}`;
  const rid = campaign.results[0].id;
  await until(
    () => api(`${campaignPath}/delivery`),
    (value) => value.counts.accepted === 1,
    'SMTP acceptance',
  );
  const inbox = await until(
    async () => {
      const response = await fetch(`${inboxBase}/api/v1/messages`, {});
      assert.equal(response.ok, true);
      return response.json();
    },
    (value) => value.messages.some((message) => message.To.some((to) => to.Address === email)),
    'matching inbox message',
  );
  const message = inbox.messages.find((item) => item.To.some((to) => to.Address === email));
  const received = await (await fetch(`${inboxBase}/api/v1/message/${message.ID}`, {})).json();
  assert(
    received.HTML.includes(`${origin}?rid=${rid}`),
    'SMTP content must bind the recipient link',
  );
  assert(
    received.HTML.includes(`/track?rid=${rid}`),
    'SMTP content must include the tracking image',
  );

  const before = await api(campaignPath);
  assert.equal(
    (await publicRequest(`/track?rid=${rid}`, { host: `${unregistered}:8443` })).status,
    404,
  );
  assert.deepEqual(await api(campaignPath), before);
  assert.equal((await publicRequest(`/track?rid=${rid}`)).status, 200);
  const landing = await publicRequest(`/?rid=${rid}`);
  assert.equal(landing.status, 200);
  assert(landing.body.includes('Hello Protocol'));
  assert(landing.body.includes('authoredScriptExecuted'));
  assert(
    landing.headers['content-security-policy'].startsWith('sandbox allow-scripts allow-forms;'),
  );
  assert(!landing.headers['content-security-policy'].includes('allow-same-origin'));
  assert(landing.headers['content-security-policy'].includes("form-action 'none'"));
  const beforeRejections = await api(campaignPath);
  const canary = `NEVER-RETAIN-${run}`;
  const event = { event_id: run.replaceAll('-', ''), password_present: true };
  for (const options of [
    { body: JSON.stringify({ ...event, password: canary }) },
    { body: JSON.stringify({ ...event, password_present: canary }) },
    { body: JSON.stringify([event, event]) },
    {
      body: `username=${canary}&password=${canary}`,
      contentType: 'application/x-www-form-urlencoded',
    },
  ]) {
    const response = await publicRequest(`/?rid=${rid}`, options);
    assert.equal(response.status, options.contentType ? 415 : 400);
    assert(!response.body.includes(canary));
    assert.deepEqual(await api(campaignPath), beforeRejections);
  }
  for (let retry = 0; retry < 2; retry++) {
    const response = await publicRequest(`/?rid=${rid}`, { body: JSON.stringify(event) });
    assert.equal(response.status, 200);
    assert.equal(JSON.parse(response.body).success, true);
  }
  const tracked = await api(campaignPath);
  for (const name of ['Email Opened', 'Clicked Link', 'Submitted Data']) {
    assert.equal(tracked.timeline.filter((item) => item.message === name).length, 1, name);
  }
  assert.equal(tracked.results[0].status, 'Submitted Data');
  assert.deepEqual(
    JSON.parse(tracked.timeline.find((item) => item.message === 'Submitted Data').details),
    { password_present: true },
  );
  assert.equal((await publicRequest(`/track?rid=${rid}`)).status, 200);
  assert.equal(
    (await api(campaignPath)).results[0].status,
    'Submitted Data',
    'Late image loads must not regress submission state',
  );
  assert(!JSON.stringify(tracked).includes(canary));
  await api(`${campaignPath}/complete`, 'POST');
  const completed = await api(campaignPath);
  assert.equal(
    (
      await publicRequest(`/?rid=${rid}`, {
        body: JSON.stringify({ ...event, event_id: 'b'.repeat(32) }),
      })
    ).status,
    404,
  );
  assert.deepEqual(await api(campaignPath), completed);
  // Inspect restricted files as their container owner, including the live SQLite WAL.
  for (const file of await readdir(data)) {
    if (
      file.startsWith('ruphish.db') ||
      file === 'ruphish.log' ||
      /^ruphish\.log\.[1-9]\d*$/.test(file)
    ) {
      const scan = spawnSync(
        'docker',
        [
          'compose',
          '-f',
          fileURLToPath(new URL('./compose.yaml', import.meta.url)),
          'exec',
          '-T',
          'app',
          'sh',
          '-c',
          'grep -aFq -- "$1" "$2"; test "$?" -eq 1',
          'sh',
          canary,
          `/lab/data/${file}`,
        ],
        { encoding: 'utf8' },
      );
      // Only grep's "absent" result succeeds; read and Docker errors fail.
      assert.equal(
        scan.status,
        0,
        `Privacy scan failed for ${file}: ${scan.error ?? (scan.stderr || 'canary retained')}`,
      );
    }
  }
});

test('container replacement preserves accounts and resources and restores Docker health', async () => {
  const before = await resourceIdentities();
  const oldContainer = docker([...compose, 'ps', '-q', 'app']);
  assert.equal(oldContainer.status, 0, oldContainer.stderr);
  const stopped = docker([...compose, 'stop', 'app']);
  assert.equal(stopped.status, 0, stopped.stderr);
  const exit = docker(['inspect', '--format={{.State.ExitCode}}', oldContainer.stdout.trim()]);
  assert.equal(exit.status, 0, exit.stderr);
  assert.equal(exit.stdout.trim(), '0', 'SIGTERM must finish cleanly within the grace period');
  const replaced = docker([
    ...compose,
    'up',
    '-d',
    '--no-deps',
    '--force-recreate',
    '--wait',
    '--wait-timeout',
    '90',
    'app',
  ]);
  assert.equal(replaced.status, 0, replaced.stderr);
  const current = docker([...compose, 'ps', '-q', 'app']);
  assert.equal(current.status, 0, current.stderr);
  assert.notEqual(current.stdout.trim(), oldContainer.stdout.trim());
  const health = docker(['inspect', '--format={{.State.Health.Status}}', current.stdout.trim()]);
  assert.equal(health.status, 0, health.stderr);
  assert.equal(health.stdout.trim(), 'healthy');
  assert.deepEqual(await resourceIdentities(), before);
  // Successful authenticated reads also prove that the persisted API token survived.
  assert.equal((await publicRequest('/health/ready')).status, 200);
});
