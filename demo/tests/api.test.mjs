import test from 'node:test';
import assert from 'node:assert/strict';
import { createDemoApi } from '../../crates/ruphish-ui/web/demo-api.mjs';
import { dataset, draftId } from '../../assets/website-demo/dataset.mjs';
import { addWorkingTime, recipientHistory } from '../../assets/website-demo/timing.mjs';
import { startRequestWith } from '../../crates/ruphish-ui/web/http.mjs';
import { readFileSync } from 'node:fs';

const now = 1793016000;

test('working-time delays cross evenings, weekends, month boundaries and DST dates in UTC', () => {
  const stamp = (value) => Date.parse(value) / 1000;
  for (const [start, seconds, expected] of [
    ['2026-10-28T16:50:00Z', 1800, '2026-10-29T09:20:00Z'],
    ['2026-10-23T16:50:00Z', 1800, '2026-10-26T09:20:00Z'],
    ['2026-03-27T16:50:00Z', 1800, '2026-03-30T09:20:00Z'],
    ['2026-10-31T12:00:00Z', 1800, '2026-11-02T09:30:00Z'],
    ['2026-12-31T16:50:00Z', 1800, '2027-01-01T09:20:00Z'],
    ['2026-10-28T08:00:00Z', 60, '2026-10-28T09:01:00Z'],
    ['2026-10-28T16:50:00Z', 600, '2026-10-28T17:00:00Z'],
    ['2026-10-28T17:00:00Z', 1, '2026-10-29T09:00:01Z'],
  ]) {
    assert.equal(addWorkingTime(stamp(start), seconds), stamp(expected));
  }
  assert.throws(() => addWorkingTime(stamp('2026-10-28T09:00:00Z'), -1), /Invalid/);
});

test('campaigns retain weekday schedules, distinct sending and tracking windows, and coherent histories', () => {
  const sample = dataset(Date.parse('2026-10-28T12:00:00Z') / 1000);
  assert.deepEqual(
    sample.campaigns.map((c) => [
      c.header.launch_date,
      c.header.send_by_date,
      new Date(c.end * 1000).toISOString(),
    ]),
    [
      ['2026-09-08T09:15:00.000Z', '2026-09-08T10:00:00.000Z', '2026-09-15T17:00:00.000Z'],
      ['2026-10-06T09:00:00.000Z', '2026-10-06T15:00:00.000Z', '2026-10-16T17:00:00.000Z'],
      ['2026-10-26T10:00:00.000Z', '2026-10-26T13:00:00.000Z', '2026-11-02T17:00:00.000Z'],
      ['2026-11-03T09:30:00.000Z', '2026-11-03T10:00:00.000Z', '2026-11-10T17:00:00.000Z'],
    ],
  );
  for (const date of [
    '2026-10-26',
    '2026-10-27',
    '2026-10-28',
    '2026-10-29',
    '2026-10-30',
    '2026-10-31',
    '2026-11-01',
    '2026-03-29',
    '2027-01-01',
  ]) {
    const anchor = Date.parse(`${date}T00:00:00Z`) / 1000;
    const data = dataset(anchor);
    const ids = new Set();
    assert.deepEqual(
      data.campaigns.map((c) => c.header.status),
      ['Completed', 'Completed', 'In progress', 'Queued'],
    );
    for (const c of data.campaigns) {
      const launch = Date.parse(c.header.launch_date) / 1000;
      const sendBy = Date.parse(c.header.send_by_date) / 1000;
      assert.ok(![0, 6].includes(new Date(launch * 1000).getUTCDay()));
      assert.ok(sendBy > launch && c.end > sendBy);
      assert.ok(c.created < launch && c.created < anchor);
      for (const r of c.results) {
        assert.ok(r.due >= launch && r.due < sendBy);
        assert.ok(Date.parse(r.modified_date) / 1000 < anchor);
        assert.equal(
          r.events.filter((e) => e.kind === 'Email Sent').length,
          r.state === 'accepted' ? 1 : 0,
        );
        if (r.state !== 'accepted') {
          assert.deepEqual(r.events, []);
        }
        if (r.state === 'ready') {
          assert.deepEqual(r.attempts, []);
        }
        const first = (kind) => r.events.find((e) => e.kind === kind);
        const click = first('Clicked Link');
        const open = first('Email Opened');
        const report = first('Email Reported');
        let previous = -Infinity;
        for (const e of r.events) {
          const at = Date.parse(e.time) / 1000;
          assert.ok(!ids.has(e.id));
          ids.add(e.id);
          assert.ok(at >= previous && at >= launch && at < anchor && at <= c.end);
          if (e.kind !== 'Email Sent') {
            assert.ok(at > Date.parse(first('Email Sent').time) / 1000);
          }
          if (e.kind === 'Clicked Link') {
            assert.ok(open && e.time > open.time);
          }
          if (e.kind === 'Submitted Data') {
            assert.ok(click && e.time > click.time);
          }
          if (report && ['Clicked Link', 'Submitted Data'].includes(e.kind)) {
            assert.ok(e.time < report.time);
          }
          previous = at;
        }
        if (r.events.length) {
          assert.equal(r.modified_date, r.events.at(-1).time);
          assert.equal(Date.parse(first('Email Sent').time) / 1000, r.attempts[0].finished);
        }
      }
    }
  }
});

test('first engagement peaks early with a multi-day tail, while repeats have bounded causal identities', () => {
  const sent = Date.parse('2026-10-19T09:00:00Z') / 1000;
  const end = Date.parse('2026-10-26T17:00:00Z') / 1000;
  let early = 0;
  let late = 0;
  let reopen = 0;
  let reclick = 0;
  let resubmit = 0;
  let reopenedAfterSubmit = 0;
  const firstTimes = new Set();
  for (let n = 0; n < 1000; n++) {
    const input = {
      key: `distribution-test/${n}`,
      sent,
      end,
      opened: true,
      clicked: true,
      submitted: true,
      reported: false,
    };
    const full = recipientHistory(input, end + 1);
    const ofKind = (kind) => full.filter((e) => e.kind === kind);
    const opens = ofKind('Email Opened');
    const clicks = ofKind('Clicked Link');
    const submits = ofKind('Submitted Data');
    firstTimes.add(opens[0].at);
    if (opens[0].at <= sent + 8 * 3600) {
      early++;
    }
    if (opens[0].at >= sent + 2 * 86400) {
      late++;
    }
    if (opens.length > 1) {
      reopen++;
    }
    if (clicks.length > 1) {
      reclick++;
    }
    if (submits.length > 1) {
      resubmit++;
    }
    if (opens.some((e) => e.at > submits[0].at)) {
      reopenedAfterSubmit++;
    }
    assert.ok(opens.length <= 4 && clicks.length <= 3 && submits.length <= 2);
    assert.equal(new Set(full.map((e) => e.slot)).size, full.length);
    assert.deepEqual(
      recipientHistory(input, sent + 86400),
      full.filter((e) => e.at < sent + 86400),
      'snapshot hides future events without changing earlier identities or times',
    );
    assert.deepEqual(
      recipientHistory({ ...input, submitted: false }, end + 1).filter(
        (e) => e.kind !== 'Submitted Data',
      ),
      full.filter((e) => e.kind !== 'Submitted Data'),
      'independent action streams do not reshuffle other events',
    );
  }
  assert.ok(early > 850 && early < 950, `first working day: ${early}/1000`);
  assert.ok(late > 20 && late < 100, `third day or later: ${late}/1000`);
  assert.ok(firstTimes.size > 800, 'responses are not synchronized to fixed offsets');
  assert.ok(reopen > 250 && reopen < 450);
  assert.ok(reclick > 80 && reclick < 220);
  assert.ok(resubmit > 20 && resubmit < 90);
  assert.ok(
    reopenedAfterSubmit > 150,
    'chronology permits returning to the email after submission',
  );
});

test('missing open observations remain absent, and impossible interaction windows fail explicitly', () => {
  const sent = Date.parse('2026-10-19T09:00:00Z') / 1000;
  const end = sent + 7 * 86400;
  const input = {
    key: 'missing-open',
    sent,
    end,
    opened: false,
    clicked: true,
    submitted: true,
    reported: true,
  };
  const events = recipientHistory(input, end + 1);
  assert.ok(!events.some((e) => e.kind === 'Email Opened'));
  assert.ok(events.find((e) => e.kind === 'Clicked Link').at > sent);
  assert.ok(
    events.find((e) => e.kind === 'Submitted Data').at >
      events.find((e) => e.kind === 'Clicked Link').at,
  );
  assert.throws(
    () => recipientHistory({ ...input, end: sent + 1 }, end + 1),
    /cannot fit before campaign closure/,
  );
});

test('the shared seed preserves completed participant targets while exposing repeats and censored active outcomes', async () => {
  const f = setup();
  await f.login();
  for (const [id, expected] of [
    [1, { accepted: 400, opened: 260, clicked: 72, submitted: 28, reported: 44 }],
    [2, { accepted: 617, opened: 320, clicked: 48, submitted: 12, reported: 140 }],
  ]) {
    const result = (await f.request(`/api/campaigns/${id}/results`)).value;
    assert.deepEqual(result.unique, expected);
    assert.ok(result.totals.opened > result.unique.opened);
    assert.ok(result.totals.clicked > result.unique.clicked);
    assert.deepEqual(result.participant_buckets.at(-1).counts, expected);
    assert.ok(result.last_observed.opened - result.first_observed.opened > 2 * 86400);
  }
  const active = (await f.request('/api/campaigns/3/results')).value;
  assert.ok(active.unique.opened > 100 && active.unique.opened < 170);
  assert.ok(active.unique.clicked > 0 && active.unique.clicked < 22);
  const selected = (await f.request('/api/campaigns/2/results?recipient=20010')).value;
  assert.equal(selected.totals.opened, 3);
  assert.equal(selected.unique.opened, 1);
  const history = (await f.request('/api/campaigns/2/recipients/20010/history')).value.events;
  assert.deepEqual(
    history.map((e) => e.message),
    ['Email Sent', 'Email Opened', 'Clicked Link', 'Clicked Link', 'Email Opened', 'Email Opened'],
  );
  const c = dataset(now).campaigns[1];
  const clicked = c.results.filter((r) => r.events.some((e) => e.kind === 'Clicked Link'));
  assert.ok(
    clicked.some((r) => r.id > 20300),
    'clickers are spread through the audience',
  );
  assert.ok(
    clicked.some((r) => r.events.some((e) => e.kind === 'Email Reported')),
    'some recipients report after clicking',
  );
});

test('completed campaign event edits use the shared closure instead of a one-day limit', async () => {
  const f = setup();
  await f.login();
  const path = '/api/campaigns/2/recipients/20010/events';
  for (const [time, status] of [
    ['2026-10-05T11:00:00Z', 200],
    ['2026-10-12T17:00:00Z', 200],
    ['2026-10-12T17:00:01Z', 400],
    ['2026-10-02T08:59:59Z', 400],
  ]) {
    const response = await f.request(path, 'POST', {
      request_id: `timing-boundary-${status}-${Date.parse(time)}`,
      action: 'add',
      event_id: null,
      revision: 0,
      value: { time, kind: 'email_opened', presence: {} },
    });
    assert.equal(response.status, status, time);
  }
});

test('source filters separate participant observations from delivery in the shared sample campaigns', async () => {
  const f = setup();
  await f.login();
  const read = async (query) => {
    const result = await f.request(`/api/campaigns/2/results?${query}`);
    assert.equal(result.status, 200);
    return result.value;
  };
  const publicEvents = await read('source=public');
  assert.deepEqual(publicEvents.totals, {
    accepted: 0,
    opened: 460,
    clicked: 54,
    submitted: 10,
    reported: 68,
  });
  assert.equal(publicEvents.event_total, 592);
  assert.ok(
    publicEvents.events.every((e) => e.source === 'public' && e.kind.value !== 'email_sent'),
  );
  const delivery = await read('source=application');
  assert.deepEqual(delivery.totals, {
    accepted: 617,
    opened: 0,
    clicked: 0,
    submitted: 0,
    reported: 0,
  });
  const selected = await read('source=public&recipients=20010&event=email_opened,link_clicked');
  assert.equal(selected.recipient_total, 1);
  assert.deepEqual(selected.totals, {
    accepted: 0,
    opened: 3,
    clicked: 2,
    submitted: 0,
    reported: 0,
  });
  assert.deepEqual(selected.unique, {
    accepted: 0,
    opened: 1,
    clicked: 1,
    submitted: 0,
    reported: 0,
  });
  const mailbox = await read('source=mailbox');
  assert.deepEqual(mailbox.totals, {
    accepted: 0,
    opened: 0,
    clicked: 0,
    submitted: 0,
    reported: 67,
  });
  assert.ok(mailbox.events.every((e) => e.kind.value === 'email_reported'));
  const operator = await read('source=operator');
  assert.deepEqual(operator.totals, {
    accepted: 0,
    opened: 31,
    clicked: 4,
    submitted: 2,
    reported: 5,
  });
  assert.equal((await read('source=legacy')).event_total, 0);
  assert.equal((await read('source=mailbox&event=link_clicked')).event_total, 0);
  for (const id of [1, 2, 3]) {
    for (const source of ['public', 'mailbox', 'operator']) {
      const result = await f.request(`/api/campaigns/${id}/results?source=${source}`);
      assert.equal(result.status, 200);
      assert.ok(result.value.event_total > 0, `campaign ${id}, source ${source}`);
    }
  }
});

test('event rows distinguish creation from editing, reject stale writes and delete directly', async () => {
  const data = dataset(now);
  const f = setup(new Map(), data);
  await f.login();
  const c = data.campaigns.find((c) => c.header.status === 'Completed');
  const r = c.results.find((r) => r.events.some((e) => e.kind === 'Clicked Link'));
  const e = r.events.find((e) => e.kind === 'Clicked Link');
  const path = `/api/campaigns/${c.header.id}/recipients/${r.id}/events`;
  const before = (await f.request(`/api/campaigns/${c.header.id}/results`)).value;
  const body = {
    request_id: 'demo-event-delete-key',
    event_id: e.id,
    revision: 0,
    action: 'delete',
    value: null,
  };
  const deleted = await f.request(path, 'POST', body);
  assert.equal(deleted.status, 200);
  assert.deepEqual(deleted.value, { id: e.id, event: null });
  const after = (await f.request(`/api/campaigns/${c.header.id}/results`)).value;
  assert.equal(after.totals.clicked, before.totals.clicked - 1);
  assert.equal((await f.request(`${path}/${e.id}`)).status, 404);
  const history = (await f.request(path.replace('/events', '/history'))).value;
  assert.ok(history.events.every((row) => row.id !== e.id));
  const add = {
    request_id: 'demo-event-add-key',
    event_id: null,
    revision: 0,
    action: 'add',
    value: { time: e.time, kind: 'email_reported', presence: {} },
  };
  const added = await f.request(path, 'POST', add);
  assert.equal(added.status, 200);
  assert.equal(added.value.event.source, 'operator');
  assert.equal(added.value.event.edited, false);
  assert.equal(added.value.event.revision, 0);
  assert.deepEqual((await f.request(path, 'POST', add)).value, added.value);
  assert.equal(
    (await f.request(path, 'POST', { ...add, value: { ...add.value, kind: 'email_opened' } }))
      .status,
    412,
  );
  assert.equal(
    (
      await f.request(path, 'POST', {
        ...add,
        request_id: 'demo-event-private-key',
        value: { ...add.value, presence: { password: 'forbidden' } },
      })
    ).status,
    400,
  );
  const edit = {
    request_id: 'demo-event-type-key',
    event_id: added.value.id,
    revision: 0,
    action: 'edit',
    value: add.value,
  };
  assert.deepEqual(
    (await f.request(path, 'POST', edit)).value,
    added.value,
    'No-op keeps the creation unedited',
  );
  edit.value = { time: e.time, kind: 'form_submitted', presence: { password_present: false } };
  const changed = await f.request(path, 'POST', edit);
  assert.equal(changed.status, 200);
  assert.equal(changed.value.event.edited, true);
  assert.equal(changed.value.event.source, 'operator');
  assert.equal(changed.value.event.revision, 1);
  assert.deepEqual((await f.request(path, 'POST', edit)).value, changed.value);
  assert.equal((await f.request(path, 'POST', { ...edit, value: add.value })).status, 412);
  const reverted = await f.request(path, 'POST', { ...edit, revision: 1, value: add.value });
  assert.equal(reverted.value.event.edited, true, 'Editing back remains marked edited');
  const results = (await f.request(`/api/campaigns/${c.header.id}/results?source=operator`)).value
    .events;
  assert.equal(results.find((e) => e.id === added.value.id).password_present, null);
  assert.equal(results.find((e) => e.id === added.value.id).edited, true);
  assert.equal(c.header.status, 'Completed');
  const removeAdded = { ...body, event_id: added.value.id, revision: 2 };
  assert.equal((await f.request(path, 'POST', removeAdded)).status, 200);
  const replacement = await f.request(path, 'POST', {
    ...add,
    request_id: 'demo-event-new-identity',
  });
  assert.ok(
    replacement.value.id > added.value.id,
    'Deleted identities cannot be assigned to new events',
  );
  assert.equal((await f.request(path, 'POST', removeAdded)).status, 404);
});

test('the full starter catalog is independent of the preloaded sample resources', async () => {
  const f = setup();
  await f.login();
  const catalog = (await f.request('/api/starter-templates/')).value;
  assert.equal(catalog.filter((row) => row.kind === 'email').length, 23);
  assert.equal(catalog.filter((row) => row.kind === 'landing_page').length, 25);
  assert.equal(catalog.filter((row) => row.kind === 'learning_page').length, 3);
  assert.equal(new Set(catalog.map((entry) => entry.id)).size, 51);
  assert.equal((await f.request('/api/templates/')).value.length, 4);
  assert.equal((await f.request('/api/pages/')).value.length, 2);
  for (const row of catalog) {
    const entry = (await f.request(`/api/starter-templates/${row.id}`)).value;
    assert.deepEqual(entry.summary, row);
    assert.equal(entry.resource.name, row.name);
    assert.equal(typeof entry.resource.html, 'string');
    assert.equal('id' in entry.resource, false);
    assert.equal('modified_date' in entry.resource, false);
    const previewPath = {
      email: '/api/util/preview',
      landing_page: '/api/pages/preview',
      learning_page: '/api/learning-pages/preview',
    }[row.kind];
    const preview = await f.request(previewPath, 'POST', { html: entry.resource.html });
    assert.equal(preview.status, 200, row.id);
    assert.equal(preview.value.html, entry.resource.html, row.id);
  }
  assert.equal((await f.request('/api/starter-templates/not-found')).status, 404);
  for (const [family, id, source] of [
    ['templates', 'email-microsoft-shared-document', 'emails/microsoft-shared-document/email.html'],
    ['pages', 'page-microsoft-side-panel', 'landing-pages/microsoft-side-panel/landing.html'],
    ['learning-pages', 'learning-slam', 'learning-pages/slam/learning.html'],
  ]) {
    const original = readFileSync(
      new URL(`../../crates/ruphish-http/starter-templates/${source}`, import.meta.url),
      'utf8',
    );
    const resource = (await f.request(`/api/${family}/1`)).value;
    const entry = (await f.request(`/api/starter-templates/${id}`)).value;
    assert.equal(resource.html, original);
    assert.equal(entry.resource.html, original);
    assert.equal(entry.summary.name, resource.name);
  }
  assert.equal(
    catalog.find((row) => row.id === 'page-microsoft-side-panel').company_logo.selector,
    '.microsoft',
  );
  assert.equal(
    catalog.find((row) => row.id === 'learning-slam').company_logo.selector,
    '.learning-logo',
  );
});

test('spreadsheet download contains separate recipient and event sheets with safe cells', async () => {
  const f = setup();
  await f.login();
  const response = await f.api.execute(
    new Request('https://demo.example.test/api/campaigns/3/export?format=xlsx&scope=all'),
  );
  assert.equal(response.status, 200);
  assert.match(response.headers.get('content-type'), /spreadsheetml/);
  const entries = await workbookEntries(response);
  assert.ok(entries.has('xl/worksheets/sheet3.xml'));
  assert.match(entries.get('xl/workbook.xml'), /name="Summary"/);
  assert.match(entries.get('xl/workbook.xml'), /name="Recipients"/);
  assert.match(entries.get('xl/workbook.xml'), /name="Events"/);
  assert.match(entries.get('xl/worksheets/sheet2.xml'), /person001@example.test/);
  assert.match(entries.get('xl/worksheets/sheet3.xml'), /Submitted Data/);
  assert.ok([...entries.values()].every((xml) => !xml.includes('<f>')));
});

async function workbookEntries(response) {
  const bytes = new Uint8Array(await response.arrayBuffer());
  const view = new DataView(bytes.buffer);
  const entries = new Map();
  let offset = 0;
  while (view.getUint32(offset, true) === 0x04034b50) {
    const length = view.getUint32(offset + 18, true);
    const nameLength = view.getUint16(offset + 26, true);
    const extra = view.getUint16(offset + 28, true);
    const start = offset + 30 + nameLength + extra;
    entries.set(
      new TextDecoder().decode(bytes.slice(offset + 30, offset + 30 + nameLength)),
      new TextDecoder().decode(bytes.slice(start, start + length)),
    );
    offset = start + length;
  }
  return entries;
}

function setup(storage = new Map(), data) {
  const store = {
    getItem: (key) => storage.get(key),
    setItem: (key, value) => storage.set(key, value),
    removeItem: (key) => storage.delete(key),
  };
  const api = createDemoApi({ now, storage: store, data });
  const request = async (path, method = 'GET', body) => {
    const response = await api.execute(
      new Request(`https://demo.example.test${path}`, {
        method,
        ...(body === undefined
          ? {}
          : { body: JSON.stringify(body), headers: { 'content-type': 'application/json' } }),
      }),
    );
    return { status: response.status, value: await response.json() };
  };
  return {
    api,
    request,
    storage,
    store,
    login: () => request('/auth/login', 'POST', { username: 'demo', password: 'demodemo' }),
  };
}

test('recipient CSV downloads retain fields, stored dates and all selected pages with safe spreadsheet text', async (t) => {
  t.mock.method(globalThis, 'fetch', () => {
    throw new Error('No network');
  });
  const data = dataset(now);
  Object.assign(data.campaigns[2].results[0], {
    first_name: 'Élise, "E"',
    last_name: 'Müller',
    position: ' =1+1',
    send_date: '2026-10-01T14:00:00.123456789+02:00',
    modified_date: '2026-10-02T01:00:00-03:00',
    password: 'never-export-this',
    ip: 'never-export-this',
  });
  Object.assign(data.campaigns[2].results[1], {
    first_name: '',
    last_name: '@command',
    position: '\ttext',
    send_date: '0001-01-01T00:00:00Z',
    modified_date: 'invalid date',
  });
  const f = setup(new Map(), data);
  const path = `/api/campaigns/3/export?format=csv&scope=recipients&recipients=30002,30001&limit=1&offset=1&from=${now}`;
  const download = (path) => f.api.execute(new Request(`https://demo.example.test${path}`));
  assert.equal((await download(path)).status, 401);
  await f.login();
  const response = await download(path);
  assert.equal(response.status, 200);
  assert.equal(
    response.headers.get('content-disposition'),
    'attachment; filename="campaign-3.csv"',
  );
  assert.equal(response.headers.get('cache-control'), 'no-store');
  const content = await response.text();
  assert.ok(!content.includes('never-export-this'));
  const review = await f.request('/api/import/group/review', 'POST', { content });
  assert.equal(review.status, 200);
  assert.deepEqual(review.value.columns, [
    'Recipient ID',
    'Email',
    'Name',
    'Delivery',
    'Accepted events',
    'Open events',
    'Click events',
    'Submission events',
    'Report events',
    'Historical reported',
    'First Name',
    'Last Name',
    'Position',
    'Send Date (UTC)',
    'Modified Date (UTC)',
  ]);
  assert.deepEqual(review.value.errors, []);
  assert.deepEqual(review.value.targets, [
    {
      email: 'person001@example.test',
      first_name: 'Élise, "E"',
      last_name: 'Müller',
      position: "' =1+1",
    },
    {
      email: 'person002@example.test',
      first_name: '',
      last_name: "'@command",
      position: "'\ttext",
    },
  ]);
  const dates = await f.request('/api/import/group/review', 'POST', {
    content,
    columns: ['Email', 'Send Date (UTC)', 'Modified Date (UTC)', 'Open events'],
  });
  assert.deepEqual(
    dates.value.targets.map((r) => [r.first_name, r.last_name, r.position]),
    [
      ['2026-10-01T12:00:00.123456789Z', '2026-10-02T04:00:00Z', '0'],
      ['', '', '0'],
    ],
  );
  const empty = await (
    await download('/api/campaigns/3/export?format=csv&search=does-not-exist')
  ).text();
  const emptyReview = await f.request('/api/import/group/review', 'POST', { content: empty });
  assert.deepEqual(emptyReview.value.columns, review.value.columns);
  assert.deepEqual(emptyReview.value.targets, []);
  for (const query of [
    'format=csv&scope=all',
    'format=unsupported',
    'scope=unknown',
    'recipients=10001',
    'recipients=bogus',
    'recipients=30001,,30002',
    `recipients=${Array(201).fill('30001').join(',')}`,
    'format=xlsx&from=not-a-time',
    'format=xlsx&from=2&until=1',
    'format=xlsx&until=1.5',
    'limit=0',
    'offset=-1',
  ]) {
    assert.equal((await download(`/api/campaigns/3/export?${query}`)).status, 400, query);
  }
  assert.equal((await download('/api/campaigns/999/export?format=csv')).status, 404);
});

test('CSV review uses supplied bytes, reports bad rows and never persists submitted recipients', async (t) => {
  t.mock.method(globalThis, 'fetch', () => {
    throw new Error('No network');
  });
  const f = setup();
  const cases = JSON.parse(
    readFileSync(new URL('../recipient-csv-cases.json', import.meta.url), 'utf8'),
  );
  const sample = readFileSync(
    new URL('../../crates/ruphish-ui/assets/recipient-template.csv', import.meta.url),
    'utf8',
  );
  assert.equal(
    (await f.request('/api/import/group/review', 'POST', { content: sample })).status,
    401,
  );
  await f.login();
  const original = (await f.request('/api/groups/')).value;
  for (const scenario of cases) {
    const body = { ...scenario.request, ...(scenario.template ? { content: sample } : {}) };
    const response = await f.request('/api/import/group/review', 'POST', body);
    assert.equal(response.status, scenario.status, scenario.name);
    if (response.status === 200) {
      assert.deepEqual(response.value.targets, scenario.targets, scenario.name);
      assert.deepEqual(
        response.value.errors.map(({ row, code }) => ({ row, code })),
        scenario.errors,
        scenario.name,
      );
    }
  }
  const imported = (await f.request('/api/import/group/review', 'POST', { content: sample })).value;
  assert.equal(
    (await f.request('/api/groups/', 'POST', { name: 'Private import', targets: imported.targets }))
      .status,
    201,
  );
  assert.deepEqual((await f.request('/api/groups/')).value, original);
  const reopened = setup(f.storage);
  assert.deepEqual((await reopened.request('/api/groups/')).value, original);
  assert.deepEqual([...f.storage.values()], ['1']);
  for (const body of [
    { content: `Email\n${'a@example.test\n'.repeat(100001)}` },
    { content: `${['Email', ...Array(1024).fill('Extra')].join(',')}\n` },
    { content: 'Email\na@example.test', columns: ['é'.repeat(513), '', '', ''] },
    { content: 'é'.repeat(4 * 1024 * 1024 + 1) },
  ]) {
    assert.ok((await f.request('/api/import/group/review', 'POST', body)).status >= 400);
  }
  assert.deepEqual(
    (await f.request('/api/import/group/review', 'POST', { content: sample })).value.targets,
    cases[0].targets,
    'failure must not poison a later review',
  );
});

test('XLSX exports preserve typed recipient cells, scope and empty schemas without formulas or inferred dates', async () => {
  const data = dataset(now);
  const first = data.campaigns[2].results[0];
  const second = data.campaigns[2].results[1];
  Object.assign(first, {
    first_name: 'Élise <E>',
    last_name: 'Müller',
    position: '=1+1',
    send_date: '2026-10-01T14:00:00.123456789+02:00',
    modified_date: '2026-10-02T01:00:00-03:00',
  });
  Object.assign(second, {
    first_name: '',
    last_name: '',
    position: '',
    send_date: null,
    modified_date: '2026-02-30T01:00:00Z',
  });
  const f = setup(new Map(), data);
  await f.login();
  const read = (query) =>
    f.api.execute(
      new Request(`https://demo.example.test/api/campaigns/3/export?format=xlsx&${query}`),
    );
  const response = await read(
    `scope=recipients&recipients=30001,30002&limit=1&offset=1&from=${now}`,
  );
  assert.equal(response.status, 200);
  const entries = await workbookEntries(response);
  const book = entries.get('xl/workbook.xml');
  assert.match(book, /name="Summary"/);
  assert.match(book, /name="Recipients"/);
  assert.doesNotMatch(book, /name="Events"/);
  const rows = [...entries.get('xl/worksheets/sheet2.xml').matchAll(/<row>(.*?)<\/row>/gs)].map(
    (m) => m[1],
  );
  assert.equal(rows.length, 3, 'both selected recipients are exported despite pagination');
  for (const header of [
    'First Name',
    'Last Name',
    'Position',
    'Send Date (UTC)',
    'Modified Date (UTC)',
  ]) {
    assert.ok(rows[0].includes(`>${header}</t>`));
  }
  assert.match(rows[1], /<c><v>30001<\/v><\/c>/);
  for (const text of [
    'Élise &lt;E&gt;',
    'Müller',
    '=1+1',
    '2026-10-01T12:00:00.123456789Z',
    '2026-10-02T04:00:00Z',
  ]) {
    assert.ok(
      rows[1].includes(`<c t="inlineStr"><is><t xml:space="preserve">${text}</t></is></c>`),
    );
  }
  assert.match(rows[2], /(?:<c t="inlineStr"><is><t xml:space="preserve"><\/t><\/is><\/c>){5}$/);
  assert.ok([...entries.values()].every((xml) => !xml.includes('<f>')));
  const empty = await workbookEntries(await read('scope=events&search=does-not-exist'));
  assert.doesNotMatch(empty.get('xl/workbook.xml'), /name="Recipients"/);
  const events = empty.get('xl/worksheets/sheet2.xml');
  assert.match(events, />Event ID<\/t>/);
  assert.match(events, />Password present<\/t>/);
  assert.equal([...events.matchAll(/<row>/g)].length, 1);
});

test('event downloads share exclusive time bounds and retain the event schema when no rows match', async () => {
  const data = dataset(now);
  const firstEvent = data.campaigns[2].results[0].events[0];
  const at = Date.parse(firstEvent.time) / 1000;
  const f = setup(new Map(), data);
  await f.login();
  const download = async (bounds) => {
    const response = await f.api.execute(
      new Request(
        `https://demo.example.test/api/campaigns/3/export?format=csv&scope=events&recipient=30001&${bounds}`,
      ),
    );
    assert.equal(response.status, 200);
    return (
      await f.request('/api/import/group/review', 'POST', {
        content: await response.text(),
        columns: ['Email', 'Event', 'Time (UTC)', 'Password present'],
      })
    ).value;
  };
  const before = await download(`until=${at}`);
  const during = await download(`from=${at}&until=${at + 1}`);
  assert.deepEqual(before.columns, [
    'Event ID',
    'Recipient ID',
    'Email',
    'Time (UTC)',
    'Event',
    'Source',
    'Password present',
    'Edited',
    'Browser',
    'Operating system',
    'Device',
  ]);
  assert.deepEqual(before.targets, []);
  assert.deepEqual(during.targets, [
    {
      email: 'person001@example.test',
      first_name: 'Email Sent',
      last_name: firstEvent.time,
      position: '',
    },
  ]);
  const analysis = await f.request(
    `/api/campaigns/3/results?recipient=30001&from=${at}&until=${at + 1}`,
  );
  assert.equal(analysis.value.event_total, 1);
});

test('login survives a fresh page, stores no password, and logout revokes the demo session', async () => {
  const f = setup();
  assert.equal((await f.request('/auth/session')).value.user, null);
  assert.equal((await f.request('/api/overview')).status, 401);
  assert.equal(
    (await f.request('/auth/login', 'POST', { username: 'demo', password: 'wrong' })).status,
    401,
  );
  assert.equal((await f.login()).status, 200);
  assert.equal((await setup(f.storage).request('/auth/session')).value.user.username, 'demo');
  assert.deepEqual([...f.storage.values()], ['1']);
  await f.request('/auth/logout', 'POST');
  assert.equal((await setup(f.storage).request('/api/groups/summary')).status, 401);
});

test('login rejects unsafe next destinations and reports unavailable storage', async () => {
  const f = setup();
  for (const next of ['//evil.example', '/\\evil.example', 'https://evil.example', '/login']) {
    assert.equal(
      (await f.request('/auth/login', 'POST', { username: 'demo', password: 'demodemo', next }))
        .value.data,
      '/',
    );
  }
  assert.equal(
    (
      await f.request('/auth/login', 'POST', {
        username: 'demo',
        password: 'demodemo',
        next: '/groups/1',
      })
    ).value.data,
    '/groups/1',
  );
  const api = createDemoApi({ now });
  const response = await api.execute(
    new Request('https://demo.example.test/auth/login', {
      method: 'POST',
      body: new URLSearchParams({ username: 'demo', password: 'demodemo' }),
    }),
  );
  assert.equal(response.status, 503);
  assert.equal((await response.json()).code, 'DEMO_STORAGE_REQUIRED');
});

test('all resource mutations resolve to real sample IDs without changing any read', async () => {
  const f = setup();
  await f.login();
  for (const family of [
    'groups',
    'templates',
    'pages',
    'learning-pages',
    'smtp',
    'domains',
    'users',
    'webhooks',
    'reporting-mailboxes',
  ]) {
    const before = (await f.request(`/api/${family}/`)).value;
    const created = await f.request(`/api/${family}/`, 'POST', {
      name: 'Discarded input',
      username: 'discarded',
      password: 'do-not-retain',
    });
    assert.equal(created.status, 201, family);
    assert.ok(
      before.some((r) => r.id === created.value.id),
      family,
    );
    const path = `/api/${family}/${created.value.id}`;
    assert.equal(
      (await f.request(path, 'PUT', { name: 'Changed', username: 'changed' })).status,
      200,
      family,
    );
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
  const f = setup();
  await f.login();
  const overview = (await f.request('/api/overview')).value;
  assert.equal(overview.recent.length, 4);
  assert.equal(overview.inventory.groups, 6);
  assert.equal(overview.attention.delivery_problems, 12);
  assert.equal((await f.request('/api/overview/posture?window=30d')).value.delivered, 937);
  assert.equal((await f.request('/api/campaigns/directory?filter=drafts')).value.items.length, 2);
  const first = (await f.request('/api/campaigns/3/delivery')).value;
  const second = (await f.request(`/api/campaigns/3/delivery?after=${first.next_after}`)).value;
  assert.equal(first.items.length, 50);
  assert.ok(second.items.every((r) => r.id > first.next_after));
  const recipient = first.items[0];
  const analysis = (await f.request(`/api/campaigns/3/results?recipient=${recipient.id}`)).value;
  assert.equal(analysis.recipients.length, 1);
  assert.ok(analysis.events.every((e) => e.recipient_id === recipient.id));
  assert.equal(
    (await f.request(`/api/campaigns/3/recipients/${recipient.id}/history`)).value.email,
    recipient.email,
  );
  assert.equal(
    (await f.request('/api/campaigns/3/results?search=does-not-exist')).value.recipient_total,
    0,
  );
});

test('analysis navigation lists campaign executions and opens their populated results', async () => {
  const f = setup();
  await f.login();
  const directory = await f.request('/api/campaigns/directory?filter=executions&after=');
  assert.equal(directory.status, 200);
  assert.equal(directory.value.items.length, 4);
  assert.ok(directory.value.items.every((row) => row.kind === 'campaign'));
  for (const row of directory.value.items) {
    const result = await f.request(`/api/campaigns/${row.id}/results`);
    assert.equal(result.status, 200);
    assert.equal(String(result.value.id), row.id);
    assert.equal(result.value.name, row.name);
    assert.ok(result.value.audience > 0);
    if (row.status !== 'Queued') {
      assert.ok(result.value.event_total > 0);
      assert.ok(result.value.recipients.length > 0);
    }
  }
});

test('saved draft, review, launch and reopening are coherent without persistence', async () => {
  const f = setup();
  await f.login();
  const path = `/api/campaign-drafts/${draftId(1)}`;
  const original = (await f.request(path)).value;
  const newPath = '/api/campaign-drafts/11111111-1111-4111-8111-111111111111';
  const created = await f.request(newPath, 'PUT', {
    revision: null,
    step: 1,
    configuration: original.configuration,
  });
  assert.equal(created.value.id, original.id);
  assert.equal((await f.request(newPath)).status, 404);
  assert.equal((await f.request(`/api/campaign-drafts/${created.value.id}`)).status, 200);
  const saved = (
    await f.request(path, 'PUT', {
      revision: null,
      step: 2,
      configuration: { ...original.configuration, name: 'Discarded campaign' },
    })
  ).value;
  assert.equal(saved.id, original.id);
  assert.equal(saved.revision, 1);
  const reviewed = (await f.request('/api/campaign-drafts/prepare', 'POST', original.configuration))
    .value;
  assert.ok(reviewed.total > 0);
  const launch = (
    await f.request(`${path}/launch`, 'POST', { revision: 1, campaign: original.configuration })
  ).value;
  assert.equal((await f.request(`/api/campaigns/${launch.id}/delivery`)).status, 200);
  assert.deepEqual((await f.request(path)).value, original);
});

test('no network fallback, including external operations, errors and binary downloads', async (t) => {
  t.mock.method(globalThis, 'fetch', () => {
    throw new Error('Network must not be used');
  });
  const f = setup();
  await f.login();
  for (const path of [
    '/api/util/test_connection',
    '/api/util/send_test_email',
    '/api/webhooks/1/validate',
    '/api/reporting-mailboxes/1/check',
    '/api/domains/1/verify',
  ]) {
    assert.equal(
      (await f.request(path, 'POST', { url: 'https://external.example' })).status,
      200,
      path,
    );
  }
  assert.equal((await f.request('/api/webhooks/1/test', 'POST')).status, 404);
  assert.equal((await f.request('/api/unknown')).status, 404);
  assert.equal((await f.request('/api/groups/', 'POST', {})).status, 400);
  const pending = startRequestWith(
    f.api.execute,
    new Request('https://demo.example.test/api/campaigns/3/export?format=csv&scope=recipients'),
    1000,
    () => {},
    true,
  );
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
    assert.ok(data.templates.some((t) => t.id === c.header.template.id));
    assert.ok(data.domains.some((d) => d.id === c.header.domain_id));
    for (const event of c.results.flatMap((r) => r.events)) {
      assert.deepEqual(Object.keys(event).sort(), [
        'client',
        'email',
        'id',
        'kind',
        'password_present',
        'recipient_id',
        'source',
        'time',
      ]);
      assert.ok(event.password_present === null || typeof event.password_present === 'boolean');
      if (event.client !== null) {
        assert.equal(event.source, 'public');
        assert.ok(['Clicked Link', 'Submitted Data'].includes(event.kind));
        assert.deepEqual(Object.keys(event.client).sort(), ['browser', 'device', 'os']);
        assert.ok(
          ['chrome', 'edge', 'firefox', 'safari', 'samsung_internet', 'unknown'].includes(
            event.client.browser,
          ),
        );
        assert.ok(
          ['windows', 'macos', 'linux', 'android', 'ios', 'unknown'].includes(event.client.os),
        );
        assert.ok(['desktop', 'mobile', 'tablet', 'unknown'].includes(event.client.device));
      }
    }
  }
  assert.deepEqual(dataset(now), data);
});

test('event selection retains recipient activity within exclusive time bounds across pages and exports', async () => {
  const data = dataset(now);
  const c = data.campaigns.find((c) => c.header.id === 2);
  c.results = c.results.slice(0, 4);
  c.results.forEach((r) => {
    r.events = [];
  });
  const from = now - 10;
  for (const [id, recipient, second, kind, source = 'public'] of [
    [99001, 0, 0, 'Email Opened'],
    [99002, 0, 1, 'Submitted Data'],
    [99003, 0, 2, 'Clicked Link'],
    [99004, 0, 3, 'Submitted Data'],
    [99005, 0, 4, 'Email Reported', 'operator'],
    [99006, 0, 5, 'Email Opened'],
    [99007, 1, -1, 'Submitted Data'],
    [99008, 1, 2, 'Clicked Link'],
    [99009, 1, 5, 'Submitted Data'],
    [99010, 2, 0, 'Email Reported'],
    [99011, 2, 4, 'Clicked Link'],
  ]) {
    const r = c.results[recipient];
    r.events.push({
      id,
      recipient_id: r.id,
      email: r.email,
      time: new Date((from + second) * 1000).toISOString(),
      kind,
      source,
      password_present: null,
    });
  }
  const f = setup(new Map(), data);
  await f.login();
  const path = `/api/campaigns/2/results?from=${from}&until=${from + 5}&event_sort=time&event_order=asc`;
  const read = async (query) => {
    const response = await f.request(`${path}&${query}`);
    assert.equal(response.status, 200);
    return response.value;
  };
  const single = await read('event=form_submitted');
  assert.deepEqual(
    single.recipients.map((r) => r.id),
    [20001],
  );
  assert.deepEqual(
    single.events.map((e) => e.id),
    [99001, 99002, 99003, 99004, 99005],
  );
  assert.deepEqual(single.totals, {
    accepted: 0,
    opened: 1,
    clicked: 1,
    submitted: 2,
    reported: 1,
  });
  assert.deepEqual(single.unique, {
    accepted: 0,
    opened: 1,
    clicked: 1,
    submitted: 1,
    reported: 1,
  });
  assert.deepEqual(single.recipients[0].counts, single.totals);
  assert.deepEqual(single.participant_buckets.at(-1).counts, single.unique);
  const selection = 'event=form_submitted,email_reported';
  const full = await read(selection);
  const page = await read(`${selection}&limit=1&offset=1`);
  assert.deepEqual(full.recipients.map((r) => r.id).sort(), [20001, 20003]);
  assert.equal(full.event_total, 7);
  assert.equal(page.events.length, 1);
  assert.equal(page.recipients.length, 1);
  assert.deepEqual(full.totals, { accepted: 0, opened: 1, clicked: 2, submitted: 2, reported: 2 });
  assert.deepEqual(full.unique, { accepted: 0, opened: 1, clicked: 2, submitted: 1, reported: 2 });
  for (const field of [
    'totals',
    'unique',
    'first_observed',
    'last_observed',
    'buckets',
    'participant_buckets',
  ]) {
    assert.deepEqual(page[field], full[field]);
  }
  const response = await f.api.execute(
    new Request(
      `https://demo.example.test/api/campaigns/2/export?from=${from}&until=${from + 5}&${selection}&event_order=asc&limit=1&offset=1&format=csv&scope=events`,
    ),
  );
  assert.equal(response.status, 200);
  assert.deepEqual(
    (await response.text())
      .trim()
      .split(/\r?\n/)
      .slice(1)
      .map((line) => Number(JSON.parse(line.split(',')[0]))),
    [99001, 99010, 99002, 99003, 99004, 99005, 99011],
  );
  assert.deepEqual((await read('event=form_submitted&source=public')).totals, {
    accepted: 0,
    opened: 1,
    clicked: 1,
    submitted: 2,
    reported: 0,
  });
  for (const restriction of [
    'source=operator',
    'recipients=20002,20003',
    'recipient=20002',
    'search=does-not-exist',
  ]) {
    const empty = await read(`event=form_submitted&${restriction}`);
    assert.equal(empty.recipient_total, 0);
    assert.equal(empty.event_total, 0);
    assert.deepEqual(empty.participant_buckets, []);
  }
  assert.equal((await read('')).recipient_total, 4);
  assert.equal((await f.request(path + '&event=invalid')).status, 400);
});

test('campaign participant series deduplicates across buckets, filters and pages while recipient counts retain repeats', async () => {
  const data = dataset(now);
  const c = data.campaigns.find((c) => c.header.id === 2);
  c.results.forEach((r) => {
    r.events = [];
  });
  const event = (id, ago, source = 'public') => ({
    id,
    time: new Date((now - ago) * 1000).toISOString(),
    kind: 'Clicked Link',
    source,
    password_present: null,
    edited: false,
  });
  c.results[0].events = [event(99001, 7200), event(99002, 3600), event(99003, 60)];
  c.results[1].events = [event(99004, 1800, 'operator')];
  for (const r of c.results) {
    for (const e of r.events) {
      Object.assign(e, { recipient_id: r.id, email: r.email });
    }
  }
  const f = setup(new Map(), data);
  await f.login();
  const read = async (query) => (await f.request(`/api/campaigns/2/results?${query}`)).value;
  const result = await read('limit=1&offset=1');
  assert.equal(result.events.length, 1);
  assert.equal(result.totals.clicked, 4);
  assert.equal(result.unique.clicked, 2);
  assert.equal(result.participant_buckets[0].counts.clicked, 1);
  assert.equal(result.participant_buckets.at(-1).counts.clicked, 2);
  const individual = await read(`recipient=${c.results[0].id}`);
  assert.equal(individual.totals.clicked, 3);
  assert.ok(individual.participant_buckets.every((b) => b.counts.clicked === 1));
  const filtered = await read(`source=public&from=${now - 4000}&until=${now}&limit=1`);
  assert.equal(filtered.totals.clicked, 2);
  assert.ok(filtered.participant_buckets.every((b) => b.counts.clicked === 1));
  assert.equal(filtered.participant_buckets[0].time, Math.floor((now - 3600) / 3600) * 3600);
  assert.deepEqual((await read('event=form_submitted')).participant_buckets, []);
});

test('client categories remain read-only across demo history, edits and exports', async () => {
  const data = dataset(now);
  const campaign = data.campaigns[1];
  const recipient = campaign.results.find((r) => r.id === 20024);
  const event = recipient.events.find((e) => e.kind === 'Clicked Link');
  assert.deepEqual(event.client, { browser: 'edge', os: 'windows', device: 'desktop' });
  const f = setup(new Map(), data);
  await f.login();
  const path = `/api/campaigns/${campaign.header.id}/recipients/${recipient.id}`;
  const history = await f.request(`${path}/history`);
  assert.deepEqual(history.value.events.find((e) => e.id === event.id).record.client, event.client);
  const change = {
    request_id: 'client-observation-edit',
    event_id: event.id,
    revision: 0,
    action: 'edit',
    value: {
      time: event.time,
      kind: 'form_submitted',
      presence: { username_present: null, password_present: false },
    },
  };
  const injected = structuredClone(change);
  injected.value.client = { browser: 'chrome', os: 'windows', device: 'desktop' };
  assert.equal((await f.request(`${path}/events`, 'POST', injected)).status, 400);
  const edited = await f.request(`${path}/events`, 'POST', change);
  assert.equal(edited.status, 200);
  assert.deepEqual(edited.value.event.client, event.client);
  const response = await f.api.execute(
    new Request(
      `https://demo.example.test/api/campaigns/${campaign.header.id}/export?format=csv&scope=events&recipient=${recipient.id}`,
    ),
  );
  const csv = await response.text();
  assert.ok(csv.includes('"edge","windows","desktop"'));
});
