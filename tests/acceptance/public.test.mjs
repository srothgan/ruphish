// Protocol acceptance against the real Docker app, DNS, TLS and SMTP services.
// Browser enforcement remains a separate manual acceptance gate.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { request } from 'node:https';
import { randomUUID } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';

const data = new URL('./data/', import.meta.url);
const env = Object.fromEntries((await readFile(new URL('bootstrap.env', data), 'utf8')).trim().split('\n').map(line => line.split('=')));
const ca = await readFile(new URL('ca.pem', data));
const admin = process.env.RUPHISH_ACCEPTANCE_ADMIN ?? 'http://127.0.0.1:3334';
const inboxBase = process.env.RUPHISH_ACCEPTANCE_INBOX ?? 'http://127.0.0.1:8026';
const publicIp = process.env.RUPHISH_ACCEPTANCE_PUBLIC_IP ?? '127.0.0.1';
const origin = 'https://good.ruphish.test:8443';
async function api(path, method = 'GET', body) {
    const response = await fetch(`${admin}${path}`, {
        method, signal: AbortSignal.timeout(10_000),
        headers: { Authorization: `Bearer ${env.GOPHISH_INITIAL_ADMIN_API_TOKEN}`, 'Content-Type': 'application/json' },
        ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    });
    assert.equal(response.ok, true, `${method} ${path}: ${response.status}`);
    return response.json();
}
function publicRequest(path, { body, host = 'good.ruphish.test:8443', contentType = 'application/json' } = {}) {
    return new Promise((resolve, reject) => {
        // Pin only resolution; ordinary certificate-chain and hostname checks remain enabled.
        const req = request(`${origin}${path}`, {
            ca, lookup: (_host, options, callback) => options.all
                ? callback(null, [{ address: publicIp, family: 4 }])
                : callback(null, publicIp, 4),
            method: body === undefined ? 'GET' : 'POST', agent: false,
            headers: { Host: host, 'Content-Type': contentType },
        }, response => {
            const chunks = [];
            response.on('data', chunk => chunks.push(chunk));
            response.on('end', () => resolve({ status: response.statusCode, headers: response.headers, body: Buffer.concat(chunks).toString('utf8') }));
            response.on('error', reject);
        });
        const timer = setTimeout(() => req.destroy(new Error('Public request deadline')), 10_000);
        req.on('close', () => clearTimeout(timer));
        req.on('error', reject);
        req.end(body);
    });
}
async function until(observe, predicate, label) {
    const deadline = Date.now() + 30_000;
    do {
        const value = await observe();
        if (predicate(value)) return value;
        await delay(100);
    } while (Date.now() < deadline);
    assert.fail(`Timed out waiting for ${label}`);
}

test('verified domain -> SMTP inbox -> public events -> completion, without participant values', { timeout: 90_000 }, async () => {
    const domains = await api('/api/domains/');
    assert.equal(domains.find(item => item.hostname === 'good.ruphish.test')?.eligible, true);
    assert.equal(domains.find(item => item.hostname === 'bad.ruphish.test')?.eligible, false);
    const eligible = await api('/api/domains/validated/');
    assert(eligible.some(item => item.origin === origin));
    assert(!eligible.some(item => item.hostname === 'bad.ruphish.test'));
    const profiles = await api('/api/smtp/');
    assert.equal(profiles.find(item => item.name === 'Local inbox only')?.host, '172.30.53.20:1025', 'Fixture SMTP must still target the local inbox');

    const run = randomUUID();
    const email = `protocol-${run}@example.test`;
    const group = await api('/api/groups/', 'POST', { name: `Protocol ${run}`, targets: [{ email, first_name: 'Protocol' }] });
    const campaign = await api('/api/campaigns/', 'POST', {
        name: `Protocol ${run}`, url: origin, groups: [{ name: group.name }],
        template: { name: 'Local privacy email' }, page: { name: 'Local privacy page' }, smtp: { name: 'Local inbox only' },
    });
    const campaignPath = `/api/campaigns/${campaign.id}`;
    const rid = campaign.results[0].id;
    await until(() => api(`${campaignPath}/delivery`), value => value.counts.accepted === 1, 'SMTP acceptance');
    const inbox = await until(async () => {
        const response = await fetch(`${inboxBase}/api/v1/messages`, { signal: AbortSignal.timeout(5000) });
        assert.equal(response.ok, true);
        return response.json();
    }, value => value.messages.some(message => message.To.some(to => to.Address === email)), 'matching inbox message');
    const message = inbox.messages.find(item => item.To.some(to => to.Address === email));
    const received = await (await fetch(`${inboxBase}/api/v1/message/${message.ID}`, { signal: AbortSignal.timeout(5000) })).json();
    assert(received.HTML.includes(`${origin}?rid=${rid}`), 'SMTP content must bind the recipient link');
    assert(received.HTML.includes(`/track?rid=${rid}`), 'SMTP content must include the tracking image');

    const before = await api(campaignPath);
    assert.equal((await publicRequest(`/track?rid=${rid}`, { host: 'bad.ruphish.test:8443' })).status, 404);
    assert.deepEqual(await api(campaignPath), before);
    assert.equal((await publicRequest(`/track?rid=${rid}`)).status, 200);
    const landing = await publicRequest(`/?rid=${rid}`);
    assert.equal(landing.status, 200);
    assert(landing.body.includes('Hello Protocol'));
    assert(!landing.body.includes('authoredScriptExecuted'));
    assert(landing.headers['content-security-policy'].includes("form-action 'none'"));
    const beforeRejections = await api(campaignPath);
    const canary = `NEVER-RETAIN-${run}`;
    const event = { event_id: run.replaceAll('-', ''), password_present: true };
    for (const options of [
        { body: JSON.stringify({ ...event, password: canary }) },
        { body: JSON.stringify({ ...event, password_present: canary }) },
        { body: JSON.stringify([event, event]) },
        { body: `username=${canary}&password=${canary}`, contentType: 'application/x-www-form-urlencoded' },
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
        assert.equal(tracked.timeline.filter(item => item.message === name).length, 1, name);
    }
    assert.equal(tracked.results[0].status, 'Submitted Data');
    assert.deepEqual(JSON.parse(tracked.timeline.find(item => item.message === 'Submitted Data').details), { password_present: true });
    assert.equal((await publicRequest(`/track?rid=${rid}`)).status, 200);
    assert.equal((await api(campaignPath)).results[0].status, 'Submitted Data', 'Late image loads must not regress submission state');
    assert(!JSON.stringify(tracked).includes(canary));
    await api(`${campaignPath}/complete`);
    const completed = await api(campaignPath);
    assert.equal((await publicRequest(`/?rid=${rid}`, { body: JSON.stringify({ ...event, event_id: 'b'.repeat(32) }) })).status, 404);
    assert.deepEqual(await api(campaignPath), completed);
    // Inspect restricted files as their container owner, including the live SQLite WAL.
    for (const file of await readdir(data)) {
        if (file.startsWith('ruphish.db') || file === 'ruphish.log') {
            const scan = spawnSync('docker', [
                'compose', '-f', fileURLToPath(new URL('./compose.yaml', import.meta.url)),
                'exec', '-T', 'app', 'sh', '-c',
                'grep -aFq -- "$1" "$2"; test "$?" -eq 1', 'sh', canary, `/lab/data/${file}`,
            ], { encoding: 'utf8', timeout: 10_000 });
            // Only grep's "absent" result succeeds; read and Docker errors fail.
            assert.equal(scan.status, 0, `Privacy scan failed for ${file}: ${scan.error ?? (scan.stderr || 'canary retained')}`);
        }
    }
});
