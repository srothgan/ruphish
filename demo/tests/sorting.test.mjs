import test from 'node:test';
import assert from 'node:assert/strict';
import { createDemoApi } from '../../crates/ruphish-ui/web/demo-api.mjs';
import { dataset } from '../../assets/website-demo/dataset.mjs';

const now = 1793016000;
async function setup() {
    const data = dataset(now), seed = data.campaigns[0];
    seed.results = [
        { id: 101, email: 'z@example.test', first_name: 'Same' },
        { id: 102, email: 'a@example.test', first_name: 'Same' },
        { id: 103, email: 'c@example.test', first_name: '' },
        { id: 104, email: 'b@example.test', first_name: '\ue000' },
        { id: 105, email: 'd@example.test', first_name: '\u{1f600}' },
    ].map((row, index) => ({ ...seed.results[0], ...row, last_name: '', due: [10, 2, 2, 5, 1][index], state: ['accepted', 'failed', 'accepted', 'cancelled', 'failed'][index], attempts: [], events: [] }));
    for (const [id, result, kind] of [[201, 102, 'Email Opened'], [202, 102, 'Email Opened'], [203, 104, 'Email Opened'], [204, 105, 'Submitted Data']]) {
        const recipient = seed.results.find(r => r.id === result);
        recipient.events.push({ id, recipient_id: result, email: recipient.email, kind, source: 'public', password_present: kind === 'Submitted Data' ? true : null, time: '2026-01-01T00:00:00Z' });
    }
    const storage = new Map();
    const api = createDemoApi({ now, data, storage: { getItem: key => storage.get(key), setItem: (key, value) => storage.set(key, value), removeItem: key => storage.delete(key) } });
    const request = (path, init) => api.execute(new Request(`https://demo.example.test${path}`, init));
    await request('/auth/login', { method: 'POST', body: JSON.stringify({ username: 'demo', password: 'demodemo' }) });
    return { data, seed, request };
}

test('analysis sorts complete filtered sets and downloads, preserving identities across pages', async () => {
    const { seed, request } = await setup();
    const path = `/api/campaigns/${seed.header.id}`;
    for (const [sort, order, expected] of [
        ['name', 'asc', [103, 101, 102, 104, 105]], ['name', 'desc', [105, 104, 101, 102, 103]],
        ['opened', 'desc', [102, 104, 101, 103, 105]], ['delivery', 'asc', [101, 103, 104, 102, 105]],
    ]) {
        const ids = [];
        for (const offset of [0, 2, 4, 6]) {
            const response = await request(`${path}/analysis?sort=${sort}&order=${order}&limit=2&offset=${offset}`);
            assert.equal(response.status, 200);
            const page = await response.json(); assert.equal(page.recipient_total, 5);
            ids.push(...page.recipients.map(row => row.id));
        }
        assert.deepEqual(ids, expected);
        const csv = await (await request(`${path}/export?format=csv&scope=recipients&sort=${sort}&order=${order}&limit=2&offset=2`)).text();
        assert.deepEqual(csv.trim().split(/\r?\n/).slice(1).map(line => Number(line.split(',')[0].replaceAll('"', ''))), expected);
    }
    const filtered = await (await request(`${path}/analysis?sort=opened&order=desc&event=email_opened&limit=1&offset=1`)).json();
    assert.deepEqual(filtered.recipients.map(row => row.id), [104]); assert.equal(filtered.recipient_total, 2);
    for (const query of ['sort=unknown', 'sort=name%3BDROP%20TABLE%20results', 'order=ascending', 'event_sort=secret', 'event_order=DESC', 'event=Email%20Opened', 'event=unknown', 'source=unknown']) {
        for (const endpoint of ['analysis', 'export?format=csv&scope=recipients']) {
            const response = await request(`${path}/${endpoint}${endpoint.includes('?') ? '&' : '?'}${query}`);
            assert.equal(response.status, 400, query);
        }
    }
});

test('delivery cursor binds sort selection and retains equal-key rows after boundary deletion', async () => {
    const { seed, request } = await setup(); const path = `/api/campaigns/${seed.header.id}/delivery`;
    const response = await request(`${path}?sort=due&order=asc&limit=2`); assert.equal(response.status, 200);
    const first = await response.json(); assert.deepEqual(first.items.map(r => r.id), [105, 102]);
    const cursor = encodeURIComponent(first.next_cursor);
    seed.results = seed.results.filter(r => r.id !== 102);
    const second = await (await request(`${path}?sort=due&order=asc&limit=2&cursor=${cursor}`)).json();
    assert.deepEqual(second.items.map(r => r.id), [103, 104]);
    const third = await (await request(`${path}?sort=due&order=asc&limit=2&cursor=${encodeURIComponent(second.next_cursor)}`)).json();
    assert.deepEqual(third.items.map(r => r.id), [101]); assert.equal(third.next_cursor, null);
    for (const query of [`sort=due&order=desc&cursor=${cursor}`, `sort=email&cursor=${cursor}`, `cursor=${cursor}`, `sort=due&after=1&cursor=${cursor}`, 'sort=due&cursor=%7B', 'sort=due&limit=0']) {
        assert.equal((await request(`${path}?${query}`)).status, 400, query);
    }
});

test('directory sorting spans pages, scopes continuations to filters, and keeps numeric identities stable', async () => {
    const { data, seed, request } = await setup();
    data.drafts = [];
    data.campaigns = Array.from({ length: 53 }, (_, i) => ({ ...seed, header: { ...seed.header, id: i + 1, name: i < 2 ? 'Z' : 'Same', total: i, status: 'Completed' } }));
    const base = '/api/campaigns/directory?filter=executions&sort=name&order=asc';
    const first = await (await request(base)).json(); assert.equal(first.items.length, 50);
    assert.deepEqual(first.items.map(r => Number(r.id)), Array.from({ length: 50 }, (_, i) => i + 3));
    const next = await (await request(`${base}&after=${encodeURIComponent(first.next_after)}`)).json();
    assert.deepEqual(next.items.map(r => Number(r.id)), [53, 1, 2]); assert.equal(next.next_after, null);
    assert.equal((await request(`/api/campaigns/directory?filter=completed&sort=name&order=asc&after=${encodeURIComponent(first.next_after)}`)).status, 400);
});


test('event pages order actual instants across zones and retain nanoseconds', async () => {
    const { seed, request } = await setup();
    const events = seed.results.flatMap(row => row.events);
    for (const [index, time] of ['2026-01-01T00:00:00Z', '2026-01-01T02:00:00.000000001+02:00', '2026-01-01T00:00:00.000000002Z', '2025-12-31T23:00:00-01:00'].entries()) events[index].time = time;
    const ids = [];
    for (const offset of [0, 2]) {
        const response = await request(`/api/campaigns/${seed.header.id}/analysis?event_sort=time&event_order=asc&limit=2&offset=${offset}`);
        assert.equal(response.status, 200);
        ids.push(...(await response.json()).events.map(row => row.id));
    }
    assert.deepEqual(ids, [201, 204, 202, 203]);
});


test('reporting uses named metrics and retains unrecognized history without inventing actions', async () => {
    const { seed, request } = await setup();
    seed.results[0].events.push({ id: 205, recipient_id: 101, email: seed.results[0].email, kind: 'Proxied request', source: 'legacy', password_present: null, time: '2026-01-01T00:00:01Z' });
    const response = await request(`/api/campaigns/${seed.header.id}/analysis`);
    assert.equal(response.status, 200);
    const value = await response.json();
    assert.deepEqual(value.totals, { accepted: 0, opened: 3, clicked: 0, submitted: 1, reported: 0 });
    assert.deepEqual(value.progression, { accepted: 0, opened: 3, clicked: 1, submitted: 1, reported: 0 });
    assert.equal(value.event_total, 5);
    assert.deepEqual(value.events.find(e => e.id === 205).kind, { type: 'historical', value: 'Proxied request' });
    assert.deepEqual(value.events.find(e => e.id === 204).kind, { type: 'known', value: 'form_submitted' });
    const csv = await (await request(`/api/campaigns/${seed.header.id}/export?scope=events&format=csv`)).text();
    assert.match(csv, /Proxied request/);
    assert.match(csv, /Submitted Data/);
    assert.doesNotMatch(csv, /\[object Object\]/);
});

test('absent native transport stays null and sorts separately from unknown acceptance', async () => {
    const { seed, request } = await setup();
    seed.results.find(row => row.id === 102).state = null;
    seed.results.find(row => row.id === 105).state = 'unknown';
    const path = `/api/campaigns/${seed.header.id}`;
    for (const [order, expected] of [['asc', [102, 101, 103, 104, 105]], ['desc', [105, 104, 101, 103, 102]]]) {
        const value = await (await request(`${path}/analysis?sort=delivery&order=${order}`)).json();
        assert.deepEqual(value.recipients.map(row => row.id), expected);
        assert.equal(value.recipients.find(row => row.id === 102).delivery, null);
        assert.equal(value.recipients.find(row => row.id === 105).delivery, 'unknown');
    }
    const csv = await (await request(`${path}/export?format=csv&scope=recipients&sort=delivery`)).text();
    assert.match(csv, /"102","a@example.test","Same","historical"/);
});
