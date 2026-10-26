import test from 'node:test';
import assert from 'node:assert/strict';
import { createDemoApi } from '../../crates/ruphish-ui/web/demo-api.mjs';
import { dataset, draftId } from '../../assets/website-demo/dataset.mjs';
import { startRequestWith } from '../../crates/ruphish-ui/web/http.mjs';
import { readFileSync } from 'node:fs';

const now = 1793016000;

test('configured resources and the demo starter picker use the existing authored starters', async () => {
    const f = setup(); await f.login();
    const catalog = (await f.request('/api/starter-templates/')).value;
    assert.equal(catalog.filter(row => row.kind === 'email').length, 4);
    assert.equal(catalog.filter(row => row.kind === 'landing_page').length, 2);
    assert.equal(catalog.filter(row => row.kind === 'learning_page').length, 3);
    for (const [family, id, source] of [
        ['templates', 'email-microsoft-shared-document', 'emails/microsoft-shared-document/email.html'],
        ['pages', 'page-microsoft-side-panel', 'landing-pages/microsoft-side-panel/landing.html'],
        ['learning-pages', 'learning-slam', 'learning-pages/slam/learning.html'],
    ]) {
        const original = readFileSync(new URL(`../../crates/ruphish-http/starter-templates/${source}`, import.meta.url), 'utf8');
        const resource = (await f.request(`/api/${family}/1`)).value;
        const entry = (await f.request(`/api/starter-templates/${id}`)).value;
        assert.equal(resource.html, original);
        assert.equal(entry.resource.html, original);
        assert.equal(entry.summary.name, resource.name);
    }
    assert.equal(catalog.find(row => row.id === 'page-microsoft-side-panel').company_logo.selector, '.microsoft');
    assert.equal(catalog.find(row => row.id === 'learning-slam').company_logo.selector, '.learning-logo');
});

test('spreadsheet download contains separate recipient and event sheets with safe cells', async () => {
    const f = setup(); await f.login();
    const response = await f.api.execute(new Request('https://demo.example.test/api/campaigns/3/export?format=xlsx&scope=all'));
    assert.equal(response.status, 200);
    assert.match(response.headers.get('content-type'), /spreadsheetml/);
    const bytes = new Uint8Array(await response.arrayBuffer()), view = new DataView(bytes.buffer), entries = new Map();
    let offset = 0;
    while (view.getUint32(offset, true) === 0x04034b50) {
        const length = view.getUint32(offset + 18, true), nameLength = view.getUint16(offset + 26, true), extra = view.getUint16(offset + 28, true);
        const start = offset + 30 + nameLength + extra;
        entries.set(new TextDecoder().decode(bytes.slice(offset + 30, offset + 30 + nameLength)), new TextDecoder().decode(bytes.slice(start, start + length)));
        offset = start + length;
    }
    assert.equal(entries.size, 6);
    assert.match(entries.get('xl/workbook.xml'), /name="Recipients"/);
    assert.match(entries.get('xl/workbook.xml'), /name="Events"/);
    assert.match(entries.get('xl/worksheets/sheet1.xml'), /person001@example.test/);
    assert.match(entries.get('xl/worksheets/sheet2.xml'), /Submitted Data/);
    assert.ok([...entries.values()].every(xml => !xml.includes('<f>')));
});

function setup(storage = new Map()) {
    const store = { getItem: key => storage.get(key), setItem: (key, value) => storage.set(key, value), removeItem: key => storage.delete(key) };
    const api = createDemoApi({ now, storage: store });
    const request = async (path, method = 'GET', body) => {
        const response = await api.execute(new Request(`https://demo.example.test${path}`, { method, ...(body === undefined ? {} : { body: JSON.stringify(body), headers: { 'content-type': 'application/json' } }) }));
        return { status: response.status, value: await response.json() };
    };
    return { api, request, storage, store, login: () => request('/login', 'POST', { username: 'demo', password: 'demodemo' }) };
}

test('login survives a fresh page, stores no password, and logout revokes the demo session', async () => {
    const f = setup();
    assert.equal((await f.request('/auth/session')).value.user, null);
    assert.equal((await f.request('/api/overview')).status, 401);
    assert.equal((await f.request('/login', 'POST', { username: 'demo', password: 'wrong' })).status, 401);
    assert.equal((await f.login()).status, 200);
    assert.equal((await setup(f.storage).request('/auth/session')).value.user.username, 'demo');
    assert.deepEqual([...f.storage.values()], ['1']);
    await f.request('/logout', 'POST');
    assert.equal((await setup(f.storage).request('/api/groups/summary')).status, 401);
});

test('login rejects unsafe next destinations and reports unavailable storage', async () => {
    const f = setup();
    for (const next of ['//evil.example', '/\\evil.example', 'https://evil.example', '/login']) {
        assert.equal((await f.request('/login', 'POST', { username: 'demo', password: 'demodemo', next })).value.data, '/');
    }
    assert.equal((await f.request('/login', 'POST', { username: 'demo', password: 'demodemo', next: '/groups/1' })).value.data, '/groups/1');
    const api = createDemoApi({ now });
    const response = await api.execute(new Request('https://demo.example.test/login', { method: 'POST', body: new URLSearchParams({ username: 'demo', password: 'demodemo' }) }));
    assert.equal(response.status, 503);
    assert.equal((await response.json()).code, 'DEMO_STORAGE_REQUIRED');
});

test('all resource mutations resolve to real sample IDs without changing any read', async () => {
    const f = setup(); await f.login();
    for (const family of ['groups', 'templates', 'pages', 'learning-pages', 'smtp', 'domains', 'users', 'webhooks', 'reporting-mailboxes']) {
        const before = (await f.request(`/api/${family}/`)).value;
        const created = await f.request(`/api/${family}/`, 'POST', { name: 'Discarded input', username: 'discarded', password: 'do-not-retain' });
        assert.equal(created.status, 201, family);
        assert.ok(before.some(r => r.id === created.value.id), family);
        const path = `/api/${family}/${created.value.id}`;
        assert.equal((await f.request(path, 'PUT', { name: 'Changed', username: 'changed' })).status, 200, family);
        assert.equal((await f.request(path, 'DELETE')).value.success, true, family);
        assert.deepEqual((await f.request(`/api/${family}/`)).value, before, family);
        assert.equal((await f.request(path)).status, 200, family);
        assert.equal((await f.request(`/api/${family}/999999`)).status, 404, family);
    }
    assert.ok(!JSON.stringify([...f.storage]).includes('do-not-retain'));
    const lesson = (await f.request('/api/learning-pages/1')).value;
    assert.ok(lesson.preview_html.includes('<h1>'));
    assert.equal(lesson.preview_html, lesson.html);
});

test('dashboard, filters, pagination and recipient drill-downs share the same records', async () => {
    const f = setup(); await f.login();
    const overview = (await f.request('/api/overview')).value;
    assert.equal(overview.campaigns.total, 4);
    assert.equal(overview.inventory.groups, 6);
    assert.equal(overview.attention.delivery_problems, 12);
    assert.equal((await f.request('/api/overview/posture?window=30d')).value.delivered, 937);
    assert.equal((await f.request('/api/campaigns/directory?filter=drafts')).value.items.length, 2);
    const first = (await f.request('/api/campaigns/3/delivery')).value;
    const second = (await f.request(`/api/campaigns/3/delivery?after=${first.next_after}`)).value;
    assert.equal(first.items.length, 50);
    assert.ok(second.items.every(r => r.id > first.next_after));
    const recipient = first.items[0];
    const analysis = (await f.request(`/api/campaigns/3/analysis?recipient=${recipient.id}`)).value;
    assert.equal(analysis.recipients.length, 1);
    assert.ok(analysis.events.every(e => e.recipient_id === recipient.id));
    assert.equal((await f.request(`/api/campaigns/3/delivery/${recipient.id}/history`)).value.email, recipient.email);
    assert.equal((await f.request('/api/campaigns/3/analysis?search=does-not-exist')).value.recipient_total, 0);
});

test('saved draft, review, launch and reopening are coherent without persistence', async () => {
    const f = setup(); await f.login();
    const path = `/api/campaign-drafts/${draftId(1)}`;
    const original = (await f.request(path)).value;
    const newPath = '/api/campaign-drafts/11111111-1111-4111-8111-111111111111';
    const created = await f.request(newPath, 'PUT', { revision: null, step: 1, configuration: original.configuration });
    assert.equal(created.value.id, original.id);
    assert.equal((await f.request(newPath)).status, 404);
    assert.equal((await f.request(`/api/campaign-drafts/${created.value.id}`)).status, 200);
    const saved = (await f.request(path, 'PUT', { revision: null, step: 2, configuration: { ...original.configuration, name: 'Discarded campaign' } })).value;
    assert.equal(saved.id, original.id);
    assert.equal(saved.revision, 1);
    const reviewed = (await f.request('/api/campaign-drafts/prepare', 'POST', original.configuration)).value;
    assert.ok(reviewed.total > 0);
    const launch = (await f.request(`${path}/launch`, 'POST', { revision: 1, campaign: original.configuration })).value;
    assert.equal((await f.request(`/api/campaigns/${launch.id}/delivery`)).status, 200);
    assert.deepEqual((await f.request(path)).value, original);
});

test('no network fallback, including external operations, errors and binary downloads', async t => {
    t.mock.method(globalThis, 'fetch', () => { throw new Error('Network must not be used'); });
    const f = setup(); await f.login();
    for (const path of ['/api/util/test_connection', '/api/util/send_test_email', '/api/webhooks/1/test', '/api/reporting-mailboxes/1/check', '/api/domains/1/verify']) {
        assert.equal((await f.request(path, 'POST', { url: 'https://external.example' })).status, 200, path);
    }
    assert.equal((await f.request('/api/unknown')).status, 404);
    assert.equal((await f.request('/api/groups/', 'POST', {})).status, 400);
    const pending = startRequestWith(f.api.execute, new Request('https://demo.example.test/api/campaigns/3/export?format=csv&scope=recipients'), 1000, () => {}, true);
    const download = await pending.promise;
    assert.ok(download.file instanceof Blob);
    assert.match(await download.file.text(), /person001@example.test/);
    assert.equal(download.body, '');
});

test('one dataset has complete references and safe participant history', () => {
    const data = dataset(now);
    assert.equal(data.users.length, 6);
    assert.equal(data.webhooks.length, 3);
    assert.equal(data['reporting-mailboxes'].length, 2);
    for (const c of data.campaigns) {
        assert.equal(c.header.total, c.results.length);
        assert.ok(data.templates.some(t => t.id === c.header.template.id));
        assert.ok(data.domains.some(d => d.id === c.header.domain_id));
        for (const event of c.results.flatMap(r => r.events)) {
            assert.deepEqual(Object.keys(event).sort(), ['email', 'id', 'kind', 'password_present', 'recipient_id', 'source', 'time']);
            assert.ok(event.password_present === null || typeof event.password_present === 'boolean');
        }
    }
    assert.deepEqual(dataset(now), data);
});
