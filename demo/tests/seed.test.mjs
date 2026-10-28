import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { createServer } from 'node:net';
import { DatabaseSync } from 'node:sqlite';
import { setTimeout as delay } from 'node:timers/promises';
import { dataset } from '../../assets/website-demo/dataset.mjs';
import { seed } from '../../assets/website-demo/seed.mjs';
import { createDemoApi } from '../../crates/ruphish-ui/web/demo-api.mjs';

test('the sole dataset seeds installed SQLite schema, survives reopen, and disables outbound integrations', async () => {
    const state = await mkdtemp(join(tmpdir(), 'ruphish-demo-seed-'));
    const listener = createServer();
    listener.listen(0, '127.0.0.1'); await once(listener, 'listening');
    const port = listener.address().port;
    await new Promise(resolve => listener.close(resolve));
    const database = join(state, 'demo.db'), config = join(state, 'config.json');
    const executable = resolve('target/debug', process.platform === 'win32' ? 'ruphish.exe' : 'ruphish');
    let child, closed;
    async function stop() {
        if (!child) return;
        child.kill(); await closed; child = undefined;
    }
    async function start() {
        child = spawn(executable, ['--config', config, '--mode', 'admin', '--disable-mailer'], {
            cwd: resolve('.'), stdio: ['ignore', 'pipe', 'pipe'],
            env: { ...process.env, GOPHISH_INITIAL_ADMIN_PASSWORD: 'Synthetic-Seed-Test-Only-942!' },
        });
        closed = once(child, 'close');
        // Startup output may contain bootstrap credentials; never relay it into test output.
        child.stdout.resume(); child.stderr.resume();
        let readiness = 'no response';
        for (let attempt = 0; attempt < 600; attempt++) {
            if (child.exitCode !== null) throw new Error('Disposable seed application exited before readiness');
            try {
                const response = await fetch(`http://127.0.0.1:${port}/auth/session`, { signal: AbortSignal.timeout(1000) });
                readiness = `HTTP ${response.status}`;
                if (response.ok) return;
            } catch { /* Starting. */ }
            await delay(100);
        }
        throw new Error(`Disposable seed application startup timed out: ${readiness}`);
    }
    try {
        await writeFile(config, JSON.stringify({ admin_server: { listen_url: `127.0.0.1:${port}`, use_tls: false }, db_name: 'sqlite3', db_path: database, logging: { level: 'warn', filename: join(state, 'app.log') } }));
        await start(); await stop();
        let db = new DatabaseSync(database), apiKey;
        try {
            seed(db, 1793016000);
            apiKey = db.prepare("SELECT api_key FROM users WHERE username='demo'").get().api_key;
        } finally { db.close(); }
        await start();
        for (const path of ['/api/overview', '/api/overview/posture?window=30d', '/api/campaigns/directory', '/api/campaigns/3/delivery', '/api/campaigns/3/results', '/api/webhooks/', '/api/reporting-mailboxes/', '/api/learning-pages/1', '/api/campaign-drafts/00000000-0000-4000-8000-000000000001']) {
            const response = await fetch(`http://127.0.0.1:${port}${path}`, { headers: { Authorization: `Bearer ${apiKey}` } });
            assert.equal(response.status, 200, path);
            const value = await response.json();
            if (path === '/api/overview') assert.equal(value.recent.length, 4);
            if (path === '/api/campaigns/3/delivery') assert.equal(value.campaign.send_by_date, null);
            if (path.includes('posture')) assert.equal(value.delivered, 937);
        }
        const memory = new Map();
        const demo = createDemoApi({ now: 1793016000, storage: { getItem: k => memory.get(k), setItem: (k, v) => memory.set(k, v), removeItem: k => memory.delete(k) } });
        const post = (path, body) => demo.execute(new Request(`https://demo.example.test${path}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) }));
        assert.equal((await post('/auth/login', { username: 'demo', password: 'demodemo' })).status, 200);
        const exportPath = '/api/campaigns/3/export?format=csv&scope=recipients&recipients=30001,30002&limit=1&offset=1';
        const live = await fetch(`http://127.0.0.1:${port}${exportPath}`, { headers: { Authorization: `Bearer ${apiKey}` } });
        const simulated = await demo.execute(new Request(`https://demo.example.test${exportPath}`));
        assert.equal(live.status, 200); assert.equal(simulated.status, 200);
        const liveCsv = await live.text(), demoCsv = await simulated.text();
        for (const columns of [null, ['Email', 'Send Date (UTC)', 'Modified Date (UTC)', 'Accepted events']]) {
            const actual = await (await post('/api/import/group/review', { content: liveCsv, columns })).json();
            const sample = await (await post('/api/import/group/review', { content: demoCsv, columns })).json();
            assert.deepEqual(sample, actual, 'demo exports must match real server headers and recipient values');
            assert.equal(actual.targets.length, 2);
            assert.deepEqual(actual.errors, []);
        }
        await stop();
        db = new DatabaseSync(database);
        try {
            const count = table => db.prepare(`SELECT count(*) AS n FROM ${table}`).get().n;
            assert.equal(count('users'), 6);
            assert.equal(count('campaigns'), 4);
            const recipient = dataset(1793016000).campaigns[2].results[0];
            const stored = db.prepare('SELECT send_date,modified_date FROM results WHERE id=?').get(recipient.id);
            assert.equal(stored.send_date, recipient.send_date);
            assert.equal(stored.modified_date, recipient.modified_date);
            assert.equal(count('groups'), 6);
            assert.equal(count('webhooks'), 3);
            assert.equal(count('ruphish_mailboxes'), 2);
            assert.equal(count('ruphish_campaign_drafts'), 2);
            assert.equal(db.prepare('SELECT count(*) AS n FROM webhooks WHERE is_active<>0').get().n, 0);
            assert.equal(db.prepare('SELECT count(*) AS n FROM ruphish_mailboxes WHERE enabled<>0').get().n, 0);
            assert.equal(db.prepare('SELECT count(*) AS n FROM ruphish_domains WHERE enabled<>0').get().n, 0);
            assert.equal(db.prepare("SELECT count(*) AS n FROM ruphish_delivery WHERE state='failed'").get().n, 12);
            assert.deepEqual(db.prepare('PRAGMA foreign_key_check').all(), []);
            assert.throws(() => seed(db), /fresh disposable administrator/);
            assert.equal(count('campaigns'), 4);
        } finally { db.close(); }
    } finally {
        await stop();
        // This directory comes directly from mkdtemp, never from user-controlled path input.
        await rm(state, { recursive: true, force: true });
    }
});
