// Local demonstration fixture only. Verification always goes through Ruphish's real DNS/HTTPS path.
import { mkdir, readFile, writeFile, rename } from 'node:fs/promises';
import { randomBytes, X509Certificate, createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

const data = fileURLToPath(new URL('./data/', import.meta.url));
const hosts = ['good.ruphish.test', 'bad.ruphish.test'];
const wrong = `ruphish-verification=${'0'.repeat(64)}`;
const base = 'http://127.0.0.1:3334';

async function credentials() {
    return Object.fromEntries((await readFile(join(data, 'bootstrap.env'), 'utf8')).trim().split('\n').map(line => line.split('=')));
}
async function zone(records) {
    const path = join(data, 'ruphish.test.zone');
    let previous = 0;
    try { previous = Number((await readFile(path, 'utf8')).match(/(\d+) ; serial/)[1]); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    const serial = Math.max(Math.floor(Date.now() / 1000), previous + 1);
    const lines = [
        '$ORIGIN ruphish.test.', '$TTL 1',
        `@ IN SOA ns.ruphish.test. hostmaster.ruphish.test. (${serial} ; serial`,
        ' 60 60 3600 1)', '@ IN NS ns.ruphish.test.', 'ns IN A 172.30.53.53',
        ...hosts.flatMap(host => [
            `${host}. IN A 172.30.53.10`,
            `_ruphish-verify.${host}. IN TXT "${records[host]}"`,
        ]), '',
    ];
    await writeFile(`${path}.tmp`, lines.join('\n'));
    await rename(`${path}.tmp`, path);
}
async function publish(host, value) {
    if (!hosts.includes(host) || !/^ruphish-verification=[0-9a-f]{64}$/.test(value ?? '')) {
        throw new Error('Use good.ruphish.test or bad.ruphish.test and copy the complete TXT value from Ruphish.');
    }
    const records = JSON.parse(await readFile(join(data, 'records.json'), 'utf8'));
    records[host] = value;
    await zone(records);
    await writeFile(join(data, 'records.json'), `${JSON.stringify(records, null, 2)}\n`);
    console.log(`Published TXT for ${host}; CoreDNS reloads the zone automatically.`);
}
async function api(path, method = 'GET', body, headers = {}) {
    const env = await credentials();
    const response = await fetch(`${base}${path}`, {
        method, signal: AbortSignal.timeout(10_000),
        headers: { Authorization: `Bearer ${env.GOPHISH_INITIAL_ADMIN_API_TOKEN}`, 'Content-Type': 'application/json', ...headers },
        ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    });
    if (!response.ok) throw new Error(`Lab API ${method} ${path}: HTTP ${response.status}`);
    return response.json();
}
async function init() {
    // Create bind sources as the host user before Docker can create root-owned
    // parents. Native provider tests must still be able to write target/debug.
    await mkdir(new URL('../../target/docker/', import.meta.url), { recursive: true });
    await mkdir(data, { recursive: true });
    try {
        await writeFile(join(data, 'bootstrap.env'), `GOPHISH_INITIAL_ADMIN_PASSWORD=${randomBytes(20).toString('hex')}\nGOPHISH_INITIAL_ADMIN_API_TOKEN=${randomBytes(32).toString('hex')}\n`, { flag: 'wx', mode: 0o600 });
    } catch (error) { if (error.code !== 'EEXIST') throw error; }
    const config = {
        admin_server: { listen_url: '0.0.0.0:3333', use_tls: false, allowed_internal_hosts: ['172.30.53.20/32'] },
        phish_server: { listen_url: '0.0.0.0:8443', use_tls: true, cert_path: '/lab/data/server.pem', key_path: '/lab/data/server.key' },
        db_name: 'sqlite3', db_path: '/lab/data/ruphish.db', migrations_prefix: 'db/db_',
        logging: { filename: '', level: 'info' },
        domain_verification: { resolvers: ['172.30.53.53:53'], allowed_networks: ['172.30.53.10/32'], trusted_ca_path: '/lab/data/ca.pem', trusted_proxies: [] },
    };
    await writeFile(join(data, 'config.json'), `${JSON.stringify(config, null, 2)}\n`);
    let records;
    try { records = JSON.parse(await readFile(join(data, 'records.json'), 'utf8')); }
    catch (error) { if (error.code !== 'ENOENT') throw error; records = Object.fromEntries(hosts.map(host => [host, wrong])); }
    await writeFile(join(data, 'records.json'), `${JSON.stringify(records, null, 2)}\n`);
    await zone(records);
    console.log('Lab initialized. Existing database, credentials and TXT values are preserved.');
}
async function demo() {
    const readyDeadline = Date.now() + 30_000;
    while (true) {
        try {
            const response = await fetch(`${base}/auth/session`, { signal: AbortSignal.timeout(2000) });
            if (response.ok) break;
        } catch { /* Compose starts containers before the application is ready. */ }
        if (Date.now() >= readyDeadline) throw new Error('Admin listener did not become ready within 30 seconds. Inspect Docker app logs.');
        await delay(250);
    }
    await establishAccount();
    const settings = await api('/api/settings');
    if (!settings.policy.verification_enabled) {
        await api('/api/settings', 'PUT', { policy: { ...settings.policy, verification_enabled: true } }, { 'If-Match': String(settings.revision) });
    }
    const existing = await api('/api/domains/');
    if (existing.some(domain => domain.hostname === hosts[1] && domain.verified_until > 0)) {
        throw new Error('The bad example has already been verified. Keep it, or delete that unused domain in the UI before resetting this demonstration.');
    }
    const domains = [];
    for (const host of hosts) {
        const origin = `https://${host}:8443`;
        const domain = existing.find(item => item.origin === origin) ?? await api('/api/domains/', 'POST', {
            name: host.startsWith('good.') ? 'Local passing example' : 'Local wrong-TXT example', origin, enabled: true,
        });
        const current = await api(`/api/domains/${domain.id}/verify`, 'POST');
        domains.push(current);
        await publish(host, host.startsWith('good.') ? current.txt_value : wrong);
    }
    // Wait for the actual DNS reload, verification cooldown/worker and persisted result.
    const deadline = Date.now() + 90_000;
    while (Date.now() < deadline) {
        for (const domain of domains) await api(`/api/domains/${domain.id}/verify`, 'POST');
        const current = await Promise.all(domains.map(domain => api(`/api/domains/${domain.id}`)));
        if (current[0].eligible && current[1].code === 'domain_txt_mismatch' && !current[1].eligible) {
            const validated = await api('/api/domains/validated/');
            if (!['control', 'dns', 'routing', 'tls'].every(key => current[0][key] === true)
                || !validated.some(domain => domain.id === current[0].id)
                || validated.some(domain => domain.id === current[1].id)) {
                throw new Error('Verification evidence or campaign eligibility differs from the expected demonstration.');
            }
            console.log('PASS: good.ruphish.test is verified and campaign-eligible.');
            console.log('PASS: bad.ruphish.test is rejected with domain_txt_mismatch.');
            console.log(`Open ${base}/domains to inspect or repair the failing example.`);
            return;
        }
        await delay(2000);
    }
    throw new Error('Verification did not reach the expected states within 90 seconds. Inspect the domain view and Docker logs.');
}

async function establishAccount() {
    const env = await credentials();
    const cookies = new Map();
    async function browser(path, fields) {
        const response = await fetch(`${base}${path}`, {
            method: fields ? 'POST' : 'GET', redirect: 'manual', signal: AbortSignal.timeout(20_000),
            headers: { Accept: 'application/json', Origin: base, Cookie: [...cookies].map(([k, v]) => `${k}=${v}`).join('; '),
                ...(fields ? { 'Content-Type': 'application/x-www-form-urlencoded' } : {}) },
            ...(fields ? { body: new URLSearchParams(fields) } : {}),
        });
        for (const cookie of response.headers.getSetCookie()) {
            const [name, ...value] = cookie.split(';')[0].split('=');
            cookies.set(name, value.join('='));
        }
        if (!response.ok) throw new Error(`Lab account setup ${path}: HTTP ${response.status}. Use the current lab password or inspect the UI.`);
        return response.json();
    }
    const initial = await browser('/auth/session');
    await browser('/login', { username: 'admin', password: env.GOPHISH_INITIAL_ADMIN_PASSWORD, csrf_token: initial.csrf_token });
    const signedIn = await browser('/auth/session');
    if (signedIn.user?.password_change_required) {
        const password = randomBytes(20).toString('hex');
        await browser('/reset_password', { password, confirm_password: password, csrf_token: signedIn.csrf_token });
        env.GOPHISH_INITIAL_ADMIN_PASSWORD = password;
        await writeFile(join(data, 'bootstrap.env'), Object.entries(env).map(([name, value]) => `${name}=${value}\n`).join(''), { mode: 0o600 });
        console.log('Completed the lab administrator first-login password change through the normal HTTP workflow.');
    }
    const current = await browser('/auth/session');
    await browser('/logout', { csrf_token: current.csrf_token });
}

async function seed() {
    // Preserve user edits: named resources are created only when absent.
    for (const [path, resource] of [
        ['/api/groups/', { name: 'Local browser audience', targets: [{ email: 'browser@example.test', first_name: 'Browser', last_name: 'Tester' }] }],
        ['/api/templates/', { name: 'Local privacy email', subject: 'Local exercise for {{.FirstName}}', text: 'Local practice page: {{.URL}}', html: await readFile(new URL('./email.html', import.meta.url), 'utf8') }],
        ['/api/pages/', { name: 'Local privacy page', html: await readFile(new URL('./landing.html', import.meta.url), 'utf8'), capture_credentials: true, capture_passwords: true }],
        ['/api/smtp/', { name: 'Local inbox only', host: '172.30.53.20:1025', from_address: 'Training <training@example.test>' }],
    ]) {
        if (!(await api(path)).some(item => item.name === resource.name)) await api(path, 'POST', resource);
    }
    const profile = (await api('/api/smtp/')).find(item => item.name === 'Local inbox only');
    const group = (await api('/api/groups/')).find(item => item.name === 'Local browser audience');
    if (profile?.host !== '172.30.53.20:1025'
        || group?.targets?.length !== 1 || group.targets[0].email !== 'browser@example.test') {
        throw new Error('Fixture profile/audience was edited. Restore the local inbox and synthetic browser recipient before seeding. No campaign was launched.');
    }
    const campaign = await api('/api/campaigns/', 'POST', {
        name: `Browser acceptance ${new Date().toISOString()}`, url: 'https://good.ruphish.test:8443',
        groups: [{ name: 'Local browser audience' }], template: { name: 'Local privacy email' },
        page: { name: 'Local privacy page' }, smtp: { name: 'Local inbox only' },
    });
    await writeFile(join(data, 'manual-campaign.json'), `${JSON.stringify(campaign, null, 2)}\n`, { mode: 0o600 });
    console.log(`Created campaign ${campaign.id}. Open http://127.0.0.1:8026 for its email; ${base}/campaigns/${campaign.id} for its history.`);
}

try {
    const [command, host, value] = process.argv.slice(2);
    switch (command) {
        case 'init': await init(); break;
        case 'publish': await publish(host, value); break;
        case 'demo': await demo(); break;
        case 'seed': await seed(); break;
        case 'browser-pin': {
            const cert = new X509Certificate(await readFile(join(data, 'server.pem')));
            console.log(createHash('sha256').update(cert.publicKey.export({ type: 'spki', format: 'der' })).digest('base64'));
            break;
        }
        case 'password': console.log((await credentials()).GOPHISH_INITIAL_ADMIN_PASSWORD); break;
        case 'status':
            for (const domain of await api('/api/domains/')) console.log(`${domain.origin}: ${domain.code}; eligible=${domain.eligible}`);
            break;
        default: throw new Error('Usage: node tests/acceptance/lab.mjs init|demo|seed|status|password|browser-pin|publish HOST TXT_VALUE');
    }
} catch (error) {
    console.error(error.message);
    process.exitCode = 1;
}
