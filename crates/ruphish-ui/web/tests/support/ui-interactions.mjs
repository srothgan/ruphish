import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { setImmediate as nextTurn } from 'node:timers/promises';
import { JSDOM } from 'jsdom';
import { createDemoApi, preferences } from '../../demo-api.mjs';
import { dataset } from '../../../../../assets/website-demo/dataset.mjs';

const scenario = process.argv[2];
const returnOrigin =
  '/campaigns/2/results?source=public&event=link_clicked&from=0&until=1999999999&sort=name&order=desc&offset=1&limit=25';
const path =
  {
    'form-import': '/groups?import=1',
    'form-webhook': '/webhooks/new',
    'form-mailbox': '/reporting-mailboxes/new',
    'form-filters': returnOrigin,
    'form-group': '/groups/new',
    'form-email': '/templates/new',
    'form-page': '/pages/new',
    'form-learning': '/learning-pages/new',
    'form-smtp': '/smtp/new',
    'form-user': '/users/new',
    'form-domain': '/domains/new',
    'domain-unsupported': '/domains/1',
    'form-password': '/user/password',
    'form-preferences': '/user/settings',
    'form-settings': '/settings',
    'form-campaign': '/campaigns/new',
    'form-draft': '/campaigns/drafts/00000000-0000-4000-8000-000000000001/edit',
    'form-session': '/groups/new',
    'results-chart': '/campaigns/2/results',
    'results-untracked': '/campaigns/2/results',
    'results-refresh': '/campaigns/2/results',
    'recipient-return': returnOrigin,
    'recipient-error-return':
      '/campaigns/2/results/recipients/999999?return_to=' + encodeURIComponent(returnOrigin),
    'recipient-direct': '/campaigns/2/results/recipients/20003',
    'recipient-foreign-return':
      '/campaigns/2/results/recipients/20003?return_to=%2Fcampaigns%2F3%2Fresults%3Foffset%3D50',
    'delivery-missing': '/campaigns/2/delivery?recipient=999999',
    'delivery-refresh': '/campaigns/2/delivery',
    'event-editor': '/campaigns/2/delivery?recipient=20003',
    'event-editor-stale-403': '/campaigns/2/delivery?recipient=20003',
    'event-editor-stale-404': '/campaigns/2/delivery?recipient=20003',
    'completed-direct': '/campaigns/2/results',
    bookmark: '/campaigns/2?source=mailbox#history',
    dashboard: '/',
    directory: '/campaigns',
    invalid: '/campaigns/not-an-id',
    new: '/campaigns/new',
  }[scenario] ?? '/campaigns/2';
const dom = new JSDOM('<!doctype html><html><body></body></html>', {
  url: `https://admin.example.test${path}`,
  pretendToBeVisual: true,
});
const win = dom.window;
for (const key of Object.getOwnPropertyNames(win)) {
  if (!(key in globalThis)) {
    try {
      globalThis[key] = win[key];
    } catch {
      /* Read-only host globals. */
    }
  }
}
Object.defineProperty(globalThis, 'navigator', { value: win.navigator, configurable: true });
globalThis.addEventListener = win.addEventListener.bind(win);
globalThis.removeEventListener = win.removeEventListener.bind(win);
globalThis.ResizeObserver = class {
  observe() {}
  disconnect() {}
};
win.ResizeObserver = globalThis.ResizeObserver;
win.matchMedia = () => ({ matches: false, addEventListener() {}, removeEventListener() {} });
win.scrollTo = () => {};
win.HTMLDialogElement.prototype.showModal = function () {
  this.open = true;
};
win.HTMLDialogElement.prototype.close = function () {
  this.open = false;
};
// Node Request needs the base URL that browsers supply for relative requests.
const NativeRequest = globalThis.Request;
globalThis.Request = class extends NativeRequest {
  constructor(input, init) {
    super(typeof input === 'string' ? new URL(input, win.location.href) : input, init);
  }
};
win.Request = globalThis.Request;
win.Response = globalThis.Response;
Object.assign(preferences, { language: 'de', format_locale: 'de-DE', time_zone: 'Europe/Berlin' });
if (scenario.startsWith('form-')) {
  preferences.language = 'en';
}
const now = 1793016000;
const NativeDate = Date;
class FixtureDate extends NativeDate {
  constructor(...args) {
    super(...(args.length ? args : [now * 1000]));
  }
  static now() {
    return now * 1000;
  }
}
globalThis.Date = win.Date = FixtureDate;
const data = dataset(now);
if (scenario === 'domain-unsupported') {
  Object.assign(data.domains[0], {
    origin: 'http://[2001:db8::20]:8080',
    hostname: '2001:db8::20',
    verified_until: 0,
    eligible: false,
    control: false,
    dns: false,
    routing: false,
    tls: false,
    txt_name: '',
    txt_value: '',
    verification: { required: true, status: 'unsupported', expires_at: null, next_check_at: null },
  });
}
const campaign = data.campaigns.find((c) => c.header.id === 2);
if (scenario === 'recipient-direct' || scenario === 'event-editor') {
  const recipient = campaign.results.find((r) => r.id === 20003);
  const sample = recipient.events.at(-1);
  for (const [id, kind, client] of [
    [880001, 'Clicked Link', { browser: 'firefox', os: 'linux', device: 'desktop' }],
    [880002, 'Submitted Data', { browser: 'safari', os: 'ios', device: 'mobile' }],
  ]) {
    recipient.events.push({
      ...sample,
      id,
      kind,
      source: 'public',
      password_present: null,
      client,
    });
  }
}

if (scenario === 'results-untracked') {
  campaign.header.policy = { ...campaign.header.policy, track_opens: false };
  assert.ok(campaign.results.some((r) => r.events.some((e) => e.kind === 'Email Opened')));
}
if (scenario === 'results-chart') {
  // Real synthetic API aggregation: repeats must not become new participants.
  campaign.results = campaign.results.slice(0, 2);
  const start = Math.floor(now / 3600) * 3600 - 4 * 3600;
  const schedules = [
    [
      ['Email Opened', 0],
      ['Clicked Link', 1],
      ['Email Opened', 2],
      ['Clicked Link', 2],
    ],
    [
      ['Email Opened', 1],
      ['Email Reported', 3],
    ],
  ];
  campaign.results.forEach((recipient, index) => {
    recipient.events = schedules[index].map(([kind, hour], slot) => ({
      id: recipient.id * 100 + slot,
      recipient_id: recipient.id,
      email: recipient.email,
      time: new Date((start + hour * 3600 + 10) * 1000).toISOString(),
      kind,
      source: 'public',
      password_present: null,
      username_present: null,
      edited: false,
    }));
  });
}
campaign.header.status =
  scenario === 'active-entry'
    ? 'In progress'
    : scenario === 'queued-entry'
      ? 'Queued'
      : 'Completed';
const api = createDemoApi({ now, data, storage: { getItem: () => '1' } });
const requests = [];
const changes = [];
let failure =
  scenario === 'missing' ? 404 : scenario === 'forbidden' ? 403 : scenario === 'retry' ? 503 : null;
let hold = false;
let release;
let resultsMode = 'initial';
let deliveryMode = 'initial';
let holdDelivery = false;
let releaseDelivery;
let holdHistory = false;
let releaseHistory;
let holdSession = false;
let releaseSession;
let sessionExpired = false;
let releaseValidation;
const featureOverrides = { webhooks_enabled: true, reporting_mailboxes_enabled: true };
let failSave = false;
const writes = [];
globalThis.fetch = async (request, options) => {
  const url = new URL(request.url);
  requests.push(url.pathname + url.search);
  if (scenario.startsWith('form-') && ['POST', 'PUT'].includes(request.method)) {
    writes.push({ path: url.pathname, body: await request.clone().json() });
    if (failSave) {
      return new Response(JSON.stringify({ success: false, code: 'TEST_FAILURE' }), {
        status: 503,
      });
    }
    if (['/api/pages/validate', '/api/learning-pages/validate'].includes(url.pathname)) {
      return new Promise((resolve) => {
        releaseValidation = (warnings) => {
          releaseValidation = null;
          resolve(
            warnings === undefined
              ? new Response(JSON.stringify({ success: false, code: 'TEST_FAILURE' }), {
                  status: 503,
                })
              : new Response(JSON.stringify({ warnings })),
          );
        };
      });
    }
  }
  if (url.pathname === '/auth/session') {
    if (holdSession) {
      await new Promise((resolve) => {
        releaseSession = resolve;
      });
    }
    const response = await api.execute(request, options);
    const value = await response.json();
    value.features = { ...value.features, ...featureOverrides };
    if (sessionExpired) {
      value.user = null;
    }
    return new Response(JSON.stringify(value), {
      status: response.status,
      headers: response.headers,
    });
  }
  if (request.method === 'POST' && url.pathname.endsWith('/events')) {
    changes.push(await request.clone().json());
  }
  if (failure && url.pathname === '/api/campaigns/2/context') {
    return new Response(
      JSON.stringify({ success: false, message: 'Unavailable', code: 'TEST_FAILURE' }),
      { status: failure },
    );
  }
  if (hold && url.pathname === '/api/campaigns/2/results') {
    await new Promise((resolve) => {
      release = resolve;
    });
  }
  if (holdHistory && url.pathname === '/api/campaigns/2/recipients/20003/history') {
    holdHistory = false;
    return new Promise((resolve) => {
      releaseHistory = () =>
        resolve(
          new Response(JSON.stringify({ success: false, code: 'TEST_FAILURE' }), {
            status: Number(scenario.slice(-3)),
          }),
        );
    });
  }
  if (
    ['results-refresh', 'results-chart'].includes(scenario) &&
    url.pathname === '/api/campaigns/2/results'
  ) {
    if (resultsMode === 'failure') {
      return new Response(JSON.stringify({ success: false, code: 'TEST_FAILURE' }), {
        status: 503,
      });
    }
    const response = await api.execute(request, options);
    const value = await response.json();
    if (resultsMode === 'reordered') {
      value.recipients.reverse();
      value.recipients.find((row) => row.id === 20003).email = 'updated@example.test';
    } else if (resultsMode === 'removed') {
      value.recipients = value.recipients.filter((row) => row.id !== 20003);
    } else if (resultsMode === 'empty') {
      value.recipients = [];
      value.recipient_total = 0;
    }
    return new Response(JSON.stringify(value), {
      status: response.status,
      headers: response.headers,
    });
  }
  if (scenario === 'delivery-refresh' && url.pathname === '/api/campaigns/2/delivery') {
    if (holdDelivery) {
      await new Promise((resolve) => {
        releaseDelivery = resolve;
      });
    }
    const response = await api.execute(request, options);
    const value = await response.json();
    if (deliveryMode === 'reordered') {
      value.items.reverse();
      value.items.find((row) => row.id === 20003).email = 'updated@example.test';
    } else if (deliveryMode === 'removed') {
      value.items = value.items.filter((row) => row.id !== 20003);
    } else if (deliveryMode === 'empty') {
      value.items = [];
      value.next_after = null;
      value.next_cursor = null;
    }
    return new Response(JSON.stringify(value), {
      status: response.status,
      headers: response.headers,
    });
  }
  return api.execute(request, options);
};
win.fetch = globalThis.fetch;
const dist = new URL('../../../../../dist/admin/', import.meta.url);
const html = readFileSync(new URL('index.html', dist), 'utf8');
const script = html.match(/import init, \* as bindings from '\/(.*?)'/)[1];
const { initSync } = await import(new URL(script, dist));
initSync({ module: readFileSync(new URL(script.replace(/\.js$/, '_bg.wasm'), dist)) });

async function until(condition) {
  while (!condition()) {
    await nextTurn();
  }
}
const document = win.document;
const regions = () => [...document.querySelectorAll('.query-region')];
const settled = () =>
  regions().length > 0 &&
  regions().every((e) => e.dataset.ready === 'true' && e.getAttribute('aria-busy') === 'false');
const resultsPath = '/campaigns/2/results';

if (scenario === 'domain-unsupported') {
  await until(() => document.body.textContent.includes('2001:db8::20'));
  assert.ok(document.body.textContent.includes('DNS-Hostnamen'));
  const verify = [...document.querySelectorAll('button')].find(
    (b) => b.textContent.trim() === 'Domain verifizieren',
  );
  assert.ok(verify);
  assert.ok(verify.disabled, 'unsupported origin cannot queue verification');
  assert.ok(!document.body.textContent.includes('_ruphish-verify'));
  process.exit(0);
}

if (scenario.startsWith('form-')) {
  const selector =
    scenario === 'form-import'
      ? 'dialog textarea'
      : scenario === 'form-filters'
        ? 'form input[type="datetime-local"]'
        : scenario === 'form-preferences'
          ? 'form select'
          : '.admin-content input:not([type="file"]):not([type="checkbox"])';
  await until(
    () =>
      document.querySelector(selector) &&
      regions().every((e) => e.dataset.ready === 'true' && e.getAttribute('aria-busy') === 'false'),
  );
  // Drain request and reactive microtasks before simulating the file picker's return.
  await nextTurn();
  const input = document.querySelector(selector);
  const form =
    input.closest(scenario === 'form-import' ? 'dialog' : 'form') ??
    input.closest('.management-page');
  const authored =
    scenario === 'form-import'
      ? 'Mailadresse,First Name,Last Name,Position\nalice@example.test,Alice,Example,Analyst\n'
      : scenario === 'form-filters'
        ? '2026-10-20T12:00'
        : scenario === 'form-preferences'
          ? 'de'
          : scenario === 'form-settings'
            ? '12345'
            : 'Unfinished user input';
  if (scenario === 'form-import') {
    const picker = form.querySelector('input[type="file"]');
    const file = new win.File([authored], 'recipients.csv', { type: 'text/csv' });
    Object.defineProperty(file, 'text', { value: async () => authored });
    Object.defineProperty(picker, 'files', { value: { 0: file, length: 1, item: () => file } });
    picker.dispatchEvent(new win.Event('change', { bubbles: true }));
    await until(() => input.value === authored);
    const advanced = form.querySelector('details');
    assert.equal(advanced.open, false, 'column mapping starts collapsed');
    assert.equal(advanced.querySelector('summary').textContent, 'Advanced');
    advanced.open = true;
    const columns = [...advanced.querySelectorAll('input')];
    assert.deepEqual(
      columns.map((field) => field.value),
      ['Email', 'First Name', 'Last Name', 'Position'],
    );
    columns[0].value = 'Mailadresse';
    columns[0].dispatchEvent(new win.Event('input', { bubbles: true }));
    advanced.open = false;
    advanced.open = true;
    assert.equal(columns[0].value, 'Mailadresse', 'collapsing preserves custom mapping');
    advanced.open = false;
  } else {
    input.value = authored;
    input.dispatchEvent(
      new win.Event(input.tagName === 'SELECT' ? 'change' : 'input', { bubbles: true }),
    );
  }
  const secret = ['form-webhook', 'form-mailbox'].includes(scenario)
    ? form.querySelector('input[type="password"]')
    : null;
  if (secret) {
    secret.value = 'local-test-secret';
    secret.dispatchEvent(new win.Event('input', { bubbles: true }));
  }
  const refreshSession = async () => {
    holdSession = true;
    win.dispatchEvent(new win.Event('focus'));
    await until(() => releaseSession);
    const finish = releaseSession;
    releaseSession = null;
    holdSession = false;
    finish();
    await nextTurn();
  };
  for (const language of ['en', 'de']) {
    preferences.language = language;
    if (language === 'de' && scenario === 'form-webhook') {
      featureOverrides.reporting_mailboxes_enabled = false;
    }
    if (language === 'de' && scenario === 'form-mailbox') {
      featureOverrides.webhooks_enabled = false;
    }
    if (scenario === 'form-filters' && language === 'de') {
      preferences.time_zone = 'Asia/Tokyo';
    }
    await refreshSession();
    if (scenario === 'form-domain') {
      const policy =
        language === 'en'
          ? 'When verification is required, only DNS hostnames over HTTPS are eligible.'
          : 'Wenn die Verifizierung erforderlich ist, sind nur DNS-Hostnamen \u00fcber HTTPS zul\u00e4ssig.';
      assert.ok(document.body.textContent.includes(policy));
    }
    assert.equal(
      input.isConnected,
      true,
      `${scenario}: refreshing the session must preserve the input node`,
    );
    assert.equal(
      input.value,
      authored,
      `${scenario}: refreshing preferences must preserve unfinished input`,
    );
    assert.equal(form.isConnected, true, `${scenario}: the form must stay mounted`);
    if (secret) {
      assert.equal(secret.value, 'local-test-secret');
    }
  }
  if (['form-webhook', 'form-mailbox'].includes(scenario)) {
    featureOverrides[
      scenario === 'form-webhook' ? 'webhooks_enabled' : 'reporting_mailboxes_enabled'
    ] = false;
    await refreshSession();
    assert.equal(input.isConnected, false, 'actual feature disablement removes its form');
    assert.ok(
      document.body.textContent.includes('Diese Funktion wurde von der Administration deaktiviert'),
    );
  }
  if (scenario === 'form-session') {
    sessionExpired = true;
    await refreshSession();
    assert.equal(
      document.querySelector('.admin-shell'),
      null,
      'session expiry still removes authenticated forms',
    );
  }
  if (scenario === 'form-import') {
    form.querySelector('.import-actions button:last-child').click();
    await until(
      () =>
        win.location.pathname === '/groups/new' &&
        [...document.querySelectorAll('.recipient-table input')].some(
          (field) => field.value === 'alice@example.test',
        ),
    );
    assert.ok(
      new URL(win.location.href).searchParams.get('draft'),
      'review hands the imported draft to the create form',
    );
    assert.equal(writes.length, 1, 'review does not save a resource');
    assert.equal(writes[0].path, '/api/import/group/review');
    assert.equal(writes[0].body.content, authored);
    assert.deepEqual(writes[0].body.columns, [
      'Mailadresse',
      'First Name',
      'Last Name',
      'Position',
    ]);
  }
  if (scenario === 'form-filters') {
    assert.ok(
      form.textContent.includes('Europe/Berlin'),
      'date inputs retain their displayed interpretation',
    );
    form.querySelector('button[type="submit"]').click();
    await until(
      () =>
        new URL(win.location.href).searchParams.get('from') ===
        String(Date.parse('2026-10-20T10:00:00Z') / 1000),
    );
  }
  if (scenario === 'form-domain') {
    const origin = form.querySelector('input[type="url"]');
    origin.value = 'http://[2001:db8::20]:8080';
    origin.dispatchEvent(new win.Event('input', { bubbles: true }));
    failSave = true;
    form.querySelector('button[type="submit"]').click();
    await until(
      () => writes.length === 1 && document.querySelector('.form-message.error.has-content'),
    );
    assert.equal(input.value, authored, 'failed save preserves the draft');
    assert.equal(origin.value, 'http://[2001:db8::20]:8080');
    assert.equal(writes[0].body.name, authored);
    failSave = false;
    form.querySelector('button[type="submit"]').click();
    await until(() => writes.length === 2 && win.location.pathname === '/domains/1');
    assert.deepEqual(writes[1], writes[0], 'explicit retry sends the retained draft');
  }
  if (['form-page', 'form-learning'].includes(scenario)) {
    const { EditorView } = await import('@codemirror/view');
    const editor = EditorView.findFromDOM(form.querySelector('.cm-editor'));
    const replace = (text) =>
      editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: text } });
    const checks = [...form.querySelectorAll('button')].filter((button) =>
      button.textContent.trim().endsWith('prüfen'),
    );
    assert.equal(checks.length, 1, 'exactly one page check action');
    const check = checks[0];
    assert.ok(check.closest('.editor-toolbar'), 'page check shares the editor toolbar');
    if (scenario === 'form-page') {
      assert.equal(form.querySelector('#redirect_url').closest('.content-editor'), null);
    }
    const checkRequests = () => writes.filter((write) => write.path.endsWith('/validate'));
    const results = () => form.querySelector('.page-check-results');
    assert.ok(results().closest('.content-editor'), 'check feedback belongs to the editor');
    replace('<p>Before</p>');
    check.click();
    await until(() => releaseValidation);
    assert.equal(check.disabled, true);
    assert.equal(check.getAttribute('aria-busy'), 'true');
    check.click();
    await nextTurn();
    assert.equal(checkRequests().length, 1, 'repeated clicks do not send duplicate checks');
    assert.equal(checkRequests()[0].body.html, '<p>Before</p>');
    replace('<p>Newer unsaved document</p>');
    replace('<p>Before</p>');
    releaseValidation();
    await until(() => !check.disabled);
    assert.equal(
      form.querySelector('.form-message.error.has-content'),
      null,
      'obsolete validation errors must not return after editing back to the same content',
    );
    replace('<p>Newer unsaved document</p>');
    check.click();
    await until(() => releaseValidation);
    releaseValidation();
    await until(() => results().querySelector('.form-message.error.has-content'));
    assert.equal(
      editor.state.sliceDoc(),
      '<p>Newer unsaved document</p>',
      'failures preserve authored content',
    );
    check.click();
    await until(() => releaseValidation);
    assert.equal(
      results().querySelector('.form-message.error.has-content'),
      null,
      'retry clears the error',
    );
    releaseValidation([]);
    await until(() => results().textContent.includes('Keine Warnungen'));
    assert.equal(check.disabled, false);
    replace('<p>Changed after checking</p>');
    await until(() => !results().textContent.includes('Keine Warnungen'));
    check.click();
    await until(() => releaseValidation);
    replace('<p>Changed again while checking</p>');
    releaseValidation([]);
    await until(() => !check.disabled);
    assert.equal(
      results().textContent.includes('Keine Warnungen'),
      false,
      'late success stays hidden',
    );
    check.click();
    await until(() => releaseValidation);
    releaseValidation([{ code: 'password_disabled', control: 1 }]);
    await until(() => results().querySelector('li'));
    assert.ok(results().querySelector('li').textContent.includes('Passwortfeld ist deaktiviert'));
    assert.equal(results().textContent.includes('Keine Warnungen'), false);
    const modeButtons = form.querySelectorAll('.editor-mode button');
    modeButtons[1].click();
    await until(() => check.disabled);
    const frame = form.querySelector('.visual-frame');
    const sent = [];
    frame.contentWindow.postMessage = (data) => sent.push(data);
    const receive = (data) =>
      win.dispatchEvent(new win.MessageEvent('message', { source: frame.contentWindow, data }));
    receive({ type: 'booted' });
    const token = sent[0].token;
    receive({ type: 'opened', token });
    await until(() => !check.disabled);
    receive({ type: 'change', token, source: '<p>Visual draft</p>' });
    await nextTurn();
    check.click();
    await until(() => releaseValidation);
    assert.equal(checkRequests().at(-1).body.html, '<p>Visual draft</p>');
    releaseValidation([]);
    await until(() => !check.disabled);
    modeButtons[0].click();
    receive({ type: 'snapshot', token, source: '<p>Visual draft</p>' });
    assert.equal(editor.state.sliceDoc(), '<p>Visual draft</p>');
    assert.equal(check.isConnected, true, 'mode changes preserve the single check control');
    assert.ok(
      writes.every((write) => write.path.endsWith('/validate')),
      'checks never save the draft',
    );
  }
  process.exit(0);
}

if (scenario === 'results-untracked') {
  await until(() => settled() && document.querySelector('.recipient-outcome'));
  assert.equal(document.querySelector('.analysis-metric strong').textContent, 'Nicht erfasst');
  for (const row of document.querySelectorAll('.recipient-outcome')) {
    assert.equal(row.cells[1].textContent, '-');
    assert.equal(row.cells[1].querySelector('span').title, 'Nicht erfasst');
  }
  assert.equal(document.querySelector('.analysis-chart .series-opened'), null);
  assert.equal(document.querySelectorAll('.analysis-legend button').length, 3);
  assert.ok(document.querySelector('.analysis-chart .series-clicked'));
  process.exit(0);
}

if (scenario === 'results-chart') {
  await until(() => settled() && document.querySelectorAll('.analysis-bin').length === 4);
  const chart = document.querySelector('.analysis-chart');
  const select = chart.querySelector('select');
  const bins = () => [...chart.querySelectorAll('.analysis-bin')];
  const tooltip = () => chart.querySelector('[role="tooltip"]');
  const values = async () => {
    const counts = [];
    for (const bin of bins()) {
      bin.focus();
      await until(tooltip);
      counts.push([...tooltip().querySelectorAll('dd')].map((cell) => Number(cell.textContent)));
      bin.blur();
      await until(() => !tooltip());
    }
    return counts;
  };
  assert.equal(select.value, 'cumulative');
  assert.deepEqual(await values(), [
    [1, 0, 0, 0],
    [2, 1, 0, 0],
    [2, 1, 0, 0],
    [2, 1, 0, 1],
  ]);
  const ticks = chart.querySelector('.analysis-x-labels').textContent;
  const targets = bins().map((bin) => [bin.style.left, bin.style.width]);
  assert.deepEqual(targets, [
    ['0%', '25%'],
    ['25%', '25%'],
    ['50%', '25%'],
    ['75%', '25%'],
  ]);
  select.focus();
  bins()[0].dispatchEvent(new win.MouseEvent('mouseenter'));
  await until(tooltip);
  const before = requests.length;
  const address = win.location.href;
  select.focus();
  select.value = 'per_interval';
  select.dispatchEvent(new win.Event('change', { bubbles: true }));
  await until(
    () =>
      !tooltip() && chart.querySelector('svg').getAttribute('aria-label').includes('pro Intervall'),
  );
  assert.equal(requests.length, before, 'changing the view does not fetch results');
  assert.equal(win.location.href, address, 'chart mode is not a URL filter');
  assert.equal(chart.querySelector('.analysis-x-labels').textContent, ticks);
  assert.deepEqual(
    bins().map((bin) => [bin.style.left, bin.style.width]),
    targets,
  );
  assert.deepEqual(await values(), [
    [1, 0, 0, 0],
    [1, 1, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 1],
  ]);
  assert.deepEqual(
    [...chart.querySelectorAll('.series-opened circle')].map((p) => p.getAttribute('cx')),
    ['12.5%', '37.5%', '62.5%', '87.5%'],
  );
  assert.equal(chart.querySelectorAll('.series-opened circle')[2].getAttribute('cy'), '100%');
  const legend = [...chart.querySelectorAll('.analysis-legend button')];
  legend[1].click();
  await until(() => legend[1].getAttribute('aria-pressed') === 'false');
  select.focus();
  select.value = 'cumulative';
  select.dispatchEvent(new win.Event('change', { bubbles: true }));
  await until(() => chart.querySelector('svg').getAttribute('aria-label').includes('kumuliert'));
  assert.equal(legend[1].getAttribute('aria-pressed'), 'false');
  assert.equal(chart.querySelector('svg .series-clicked'), null);
  select.focus();
  select.value = 'per_interval';
  select.dispatchEvent(new win.Event('change', { bubbles: true }));
  await until(() =>
    chart.querySelector('svg').getAttribute('aria-label').includes('pro Intervall'),
  );
  const refresh = () => document.querySelector('.refresh-indicator').closest('button').click();
  resultsMode = 'failure';
  refresh();
  await until(() => settled() && document.querySelector('.query-refresh-error'));
  assert.equal(select.value, 'per_interval');
  assert.equal(document.querySelector('.analysis-chart'), chart);
  resultsMode = 'initial';
  const recipient = campaign.results[0];
  recipient.events.push({ ...recipient.events[1], id: 999999, kind: 'Submitted Data' });
  bins()[1].focus();
  await until(tooltip);
  refresh();
  await until(() => settled() && !document.querySelector('.query-refresh-error') && !tooltip());
  assert.equal(select.value, 'per_interval');
  assert.deepEqual(await values(), [
    [1, 0, 0],
    [1, 1, 0],
    [0, 0, 0],
    [0, 0, 1],
  ]);
  const form = document.querySelector('.analysis-filters');
  for (const source of ['operator', 'public']) {
    const filter = form.querySelector('select');
    filter.value = source;
    filter.dispatchEvent(new win.Event('change', { bubbles: true }));
    hold = true;
    form.querySelector('button[type="submit"]').click();
    await until(() => release && !settled());
    assert.equal(select.value, 'per_interval');
    hold = false;
    release();
    release = null;
    await until(() => win.location.search.includes(source) && settled());
    assert.equal(document.querySelector('.analysis-chart'), chart);
    assert.equal(select.value, 'per_interval');
    if (source === 'operator') {
      assert.equal(bins().length, 0);
    }
  }
  await until(() => bins().length === 4);
  const currentLegend = [...chart.querySelectorAll('.analysis-legend button')];
  assert.equal(currentLegend[1].getAttribute('aria-pressed'), 'false');
  for (const button of currentLegend.filter(
    (button) => button.getAttribute('aria-pressed') === 'true',
  )) {
    button.click();
  }
  await until(() => chart.querySelector('[role="status"]'));
  assert.equal(chart.querySelector('.analysis-plot').parentElement.hidden, true);
  currentLegend[0].click();
  await until(() => !chart.querySelector('[role="status"]'));
  bins()[0].focus();
  await until(tooltip);
  bins()[0].dispatchEvent(new win.KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await until(() => !tooltip());
  document.querySelector('.campaign-sections a[href="/campaigns/2/delivery"]').click();
  await until(() => win.location.pathname.endsWith('/delivery') && settled());
  document.querySelector('.campaign-sections a[href="/campaigns/2/results"]').click();
  await until(
    () =>
      win.location.pathname === resultsPath &&
      settled() &&
      document.querySelector('.analysis-chart select'),
  );
  assert.equal(document.querySelector('.analysis-chart select').value, 'cumulative');
  process.exit(0);
}

if (scenario === 'results-refresh') {
  await until(() => settled() && document.querySelectorAll('.recipient-outcome').length > 1);
  const rows = () => [...document.querySelectorAll('.recipient-outcome')];
  const id = (row) => new URL(row.querySelector('a').href).pathname.split('/').at(-1);
  const original = rows().map(id);
  const retained = rows().find((row) => id(row) === '20003');
  assert.ok(retained);
  const refresh = () => document.querySelector('.refresh-indicator').closest('button').click();
  resultsMode = 'reordered';
  refresh();
  await until(() => settled() && retained.textContent.includes('updated@example.test'));
  assert.deepEqual(rows().map(id), [...original].reverse());
  assert.equal(
    rows().find((row) => id(row) === '20003'),
    retained,
  );
  resultsMode = 'failure';
  const before = requests.length;
  refresh();
  await until(
    () => requests.length > before && settled() && document.querySelector('.query-refresh-error'),
  );
  assert.equal(
    rows().find((row) => id(row) === '20003'),
    retained,
  );
  assert.ok(retained.textContent.includes('updated@example.test'));
  resultsMode = 'removed';
  refresh();
  await until(() => settled() && !rows().some((row) => id(row) === '20003'));
  assert.deepEqual(
    rows().map(id),
    original.filter((value) => value !== '20003'),
  );
  resultsMode = 'empty';
  refresh();
  await until(() => settled() && rows().length === 0);
  process.exit(0);
}

if (scenario.startsWith('recipient-')) {
  if (scenario === 'recipient-return') {
    await until(() => settled() && document.querySelector('.recipient-outcome a'));
    const link = document.querySelector('.recipient-outcome a');
    assert.equal(new URL(link.href).searchParams.get('return_to'), returnOrigin);
    link.click();
  }
  if (scenario === 'recipient-error-return') {
    await until(() => document.querySelector('a[href="' + returnOrigin + '"]'));
  } else {
    await until(
      () =>
        win.location.pathname.startsWith(resultsPath + '/recipients/') &&
        settled() &&
        document.querySelector('a.breadcrumb'),
    );
    const recipient = win.location.pathname.split('/').at(-1);
    const history = requests
      .map((request) => new URL(request, win.location.origin))
      .filter(
        (url) =>
          url.pathname === '/api/campaigns/2/results' &&
          url.searchParams.get('recipient') === recipient,
      );
    assert.ok(history.length > 0);
    for (const url of history) {
      for (const key of ['source', 'event', 'from', 'until', 'recipients', 'return_to']) {
        assert.equal(url.searchParams.has(key), false, 'recipient history must not inherit ' + key);
      }
      assert.equal(url.searchParams.get('event_order'), 'asc');
    }
  }
  if (scenario === 'recipient-direct') {
    const observations = [...document.querySelectorAll('.recipient-observation')];
    const click = observations.find((e) =>
      e.querySelector('.client-observation')?.textContent.includes('Firefox'),
    );
    const submission = observations.find((e) =>
      e.querySelector('.client-observation')?.textContent.includes('Safari'),
    );
    assert.ok(click?.textContent.includes('Linux'));
    assert.ok(submission?.textContent.includes('iOS'));
    assert.ok(submission?.textContent.includes('Mobilger\u00e4t'));
    assert.equal(document.querySelectorAll('.client-observation dl').length, 2);
  }
  const expected = ['recipient-return', 'recipient-error-return'].includes(scenario)
    ? returnOrigin
    : resultsPath;
  const back =
    document.querySelector('a.breadcrumb') ?? document.querySelector('a[href="' + expected + '"]');
  assert.equal(back.getAttribute('href'), expected);
  back.click();
  await until(() => win.location.pathname + win.location.search === expected && settled());
  assert.ok(document.querySelector('.analysis-filters'));
  process.exit(0);
}

if (scenario === 'delivery-missing') {
  await until(() => document.querySelector('.query-failure [role="alert"]'));
  assert.equal(document.querySelectorAll('.recipient-summary').length, 0);
  assert.ok(requests.includes('/api/campaigns/2/delivery?after=999998&limit=1'));
  assert.equal(win.location.search, '?recipient=999999');
  process.exit(0);
}

if (scenario === 'delivery-refresh') {
  await until(() => settled() && document.querySelectorAll('.recipient-summary').length > 1);
  const first = document.querySelector('.recipient-toggle').id;
  const pagination = () => document.querySelectorAll('.management-footer button');
  pagination()[1].click();
  await until(() => settled() && document.querySelector('.recipient-toggle').id !== first);
  assert.ok(
    requests.some(
      (path) => path.startsWith('/api/campaigns/2/delivery?cursor=') && !path.includes('cursor=&'),
    ),
  );
  pagination()[0].click();
  await until(() => settled() && document.querySelector('.recipient-toggle').id === first);
  const toggle = document.querySelector('#recipient-toggle-20003');
  toggle.click();
  await until(() => document.querySelector('#recipient-history-20003 .delivery-timeline li'));
  const summary = toggle.closest('tr');
  const history = document.querySelector('#recipient-history-20003');
  const originalOrder = [...document.querySelectorAll('.recipient-toggle')].map(
    (button) => button.id,
  );
  const refresh = () => document.querySelector('.refresh-indicator').closest('button').click();
  deliveryMode = 'reordered';
  holdDelivery = true;
  refresh();
  await until(() => releaseDelivery);
  assert.equal(document.querySelector('#recipient-toggle-20003'), toggle);
  assert.equal(toggle.getAttribute('aria-expanded'), 'true');
  assert.equal(document.querySelector('#recipient-history-20003'), history);
  assert.ok(
    history.querySelector('.delivery-timeline li'),
    'confirmed history remains while refreshing',
  );
  holdDelivery = false;
  releaseDelivery();
  await until(() => settled() && summary.textContent.includes('updated@example.test'));
  assert.deepEqual(
    [...document.querySelectorAll('.recipient-toggle')].map((button) => button.id),
    originalOrder.toReversed(),
  );
  assert.equal(
    document.querySelector('#recipient-toggle-20003'),
    toggle,
    'reordering preserves recipient identity',
  );
  assert.equal(document.querySelector('#recipient-history-20003'), history);
  assert.equal(toggle.getAttribute('aria-expanded'), 'true');
  deliveryMode = 'removed';
  refresh();
  await until(() => settled() && !document.querySelector('#recipient-toggle-20003'));
  assert.equal(document.querySelector('#recipient-history-20003'), null);
  assert.ok(
    [...document.querySelectorAll('.recipient-toggle')].every(
      (button) => button.getAttribute('aria-expanded') === 'false',
    ),
    'removal must not transfer expansion to another recipient',
  );
  deliveryMode = 'empty';
  refresh();
  await until(() => settled() && document.querySelectorAll('.recipient-summary').length === 0);
  assert.equal(
    document.querySelector('.query-placeholder'),
    null,
    'a confirmed empty page is loaded',
  );
  process.exit(0);
}

if (scenario.startsWith('event-editor')) {
  await until(
    () => settled() && document.querySelector('.recipient-history .delivery-timeline li'),
  );
  document.querySelector('.recipient-history > .recipient-history-heading button').click();
  await until(() => document.querySelector('.event-review'));
  const review = document.querySelector('.event-review');
  if (scenario === 'event-editor') {
    assert.ok(review.textContent.includes('Firefox'));
    assert.ok(review.textContent.includes('Safari'));
    assert.equal(review.querySelectorAll('.client-observation dl').length, 2);
    const timeline = document.querySelector('.delivery-timeline');
    assert.ok(timeline.textContent.includes('Firefox'));
    assert.ok(timeline.textContent.includes('Safari'));
  }

  const recipient = campaign.results.find((r) => r.id === 20003);
  const initialCount = recipient.events.length;
  const change = (control, value, type = 'change') => {
    control.value = value;
    control.dispatchEvent(new win.Event(type, { bubbles: true }));
  };
  const openAdd = async () => {
    review.querySelector(':scope > button').click();
    await until(() => document.querySelector('form.event-correction'));
    const form = document.querySelector('form.event-correction');
    assert.equal(
      form.closest('dialog').querySelector('table').contains(form),
      false,
      'the editor is outside the dialog event table',
    );
    assert.equal(document.querySelectorAll('form.event-correction').length, 1);
    assert.equal(review.hidden, true);
    assert.equal(form.querySelector('.event-correction-actions').children.length, 2);
    return form;
  };
  let form = await openAdd();
  form.querySelector('button[type="button"]').click();
  await until(() => !document.querySelector('form.event-correction'));
  assert.equal(review.hidden, false);
  assert.equal(changes.length, 0, 'cancel never saves');
  const localMinute = (time) =>
    new Intl.DateTimeFormat('sv-SE', {
      timeZone: 'Europe/Berlin',
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      hourCycle: 'h23',
    })
      .format(new Date(time))
      .replace(' ', 'T');
  form = await openAdd();
  const occurrence = form.querySelector('input');
  assert.equal(
    occurrence.value.slice(0, 16),
    localMinute(now * 1000),
    'new events default to the current time, even in a completed campaign',
  );
  // Pick a historical occurrence before submitting to the completed fixture campaign.
  const originalTime = localMinute(recipient.events.at(-1).time);
  change(occurrence, '', 'input');
  assert.equal(form.checkValidity(), false, 'an empty occurrence time cannot submit');
  change(occurrence, originalTime, 'input');
  change(form.querySelector('select'), 'form_submitted');
  await until(() => form.querySelector('.event-correction-presence'));
  change(form.querySelectorAll('select')[1], 'true');
  change(form.querySelectorAll('select')[2], 'false');
  holdHistory = scenario.startsWith('event-editor-stale-');
  form.dispatchEvent(new win.Event('submit', { bubbles: true, cancelable: true }));
  await until(() => !document.querySelector('form.event-correction'));
  assert.equal(review.hidden, false);
  assert.equal(recipient.events.length, initialCount + 1);
  assert.equal(changes.length, 1);
  assert.equal(changes[0].action, 'add');
  const added = recipient.events.at(-1);
  assert.equal(added.source, 'operator');
  assert.equal(added.edited, false);
  assert.equal(added.revision, 0);
  const operatorRow = () =>
    [...review.querySelectorAll('tr:has(td)')].find(
      (row) => row.cells[2]?.textContent.trim() === 'Manuell erfasst',
    );
  assert.ok(operatorRow(), 'operator creation is visible');
  assert.deepEqual(
    [...operatorRow().querySelectorAll('.field-presence small')].map((e) => e.textContent),
    ['Keine Eingabe erkannt', 'Eingegeben'],
  );
  const submittedTimeline = [...document.querySelectorAll('.timeline-event')].find((row) =>
    row.querySelector('.submission-fields'),
  );
  assert.ok(submittedTimeline, 'delivery history shows submission field indicators');

  assert.equal(
    operatorRow().cells[3].textContent.trim(),
    '',
    'creating an operator event must not show Edited',
  );
  assert.deepEqual(changes[0].value.presence, { username_present: false, password_present: true });
  if (scenario.startsWith('event-editor-stale-')) {
    await until(() => releaseHistory);
  }
  const mountedHistory = document.querySelector('.recipient-history');
  operatorRow().querySelector('.table-actions button').click();
  await until(() => document.querySelector('form.event-correction'));
  form = document.querySelector('form.event-correction');
  change(form.querySelector('select'), 'email_reported');
  await until(() => !form.querySelector('.event-correction-presence'));
  form.dispatchEvent(new win.Event('submit', { bubbles: true, cancelable: true }));
  await until(() => !document.querySelector('form.event-correction'));
  assert.equal(changes.length, 2);
  assert.equal(changes[1].action, 'edit');
  assert.equal(changes[1].value.kind, 'email_reported');
  assert.deepEqual(changes[1].value.presence, { username_present: null, password_present: null });
  assert.equal(recipient.events.length, initialCount + 1, 'editing must not add another event');
  assert.equal(review.hidden, false);
  assert.equal(added.edited, true);
  assert.equal(added.source, 'operator');
  assert.equal(added.revision, 1);
  assert.equal(operatorRow().cells[3].textContent.trim(), 'Bearbeitet');
  if (releaseHistory) {
    releaseHistory();
    // Drain the Fetch/body/WASM callback microtasks and reactive effects.
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(document.querySelector('.recipient-history'), mountedHistory);
    assert.ok(operatorRow(), 'an obsolete terminal read cannot erase the confirmed edit');
    assert.equal(operatorRow().cells[3].textContent.trim(), 'Bearbeitet');
    assert.ok(mountedHistory.querySelector('.delivery-timeline li'));
    assert.ok(mountedHistory.querySelector('.recipient-attempts .delivery-attempt'));
  }
  operatorRow().querySelectorAll('.table-actions button')[1].click();
  await until(() => document.querySelector('form.event-correction'));
  form = document.querySelector('form.event-correction');
  form.dispatchEvent(new win.Event('submit', { bubbles: true, cancelable: true }));
  await until(() => !document.querySelector('form.event-correction'));
  assert.equal(changes.at(-1).action, 'delete');
  assert.equal(recipient.events.length, initialCount);
  assert.ok(recipient.events.every((e) => e.id !== added.id));
  assert.equal(review.querySelectorAll('tr:has(td)').length, initialCount);
  form = await openAdd();
  change(form.querySelector('select'), 'link_clicked');
  const dialog = form.closest('dialog');
  const close = dialog.querySelector('.recipient-history-heading button');
  assert.equal(close.disabled, false, 'an unsaved draft does not disable Close');
  const savedChanges = changes.length;
  close.click();
  const confirmation = document.querySelector('[data-discard="cancel"]').closest('dialog');
  await until(() => confirmation.open);
  assert.equal(dialog.open, true, 'the event dialog stays open behind confirmation');
  confirmation.querySelector('[data-discard="cancel"]').click();
  await until(() => !confirmation.open);
  assert.equal(form.isConnected, true);
  assert.equal(
    form.querySelector('select').value,
    'link_clicked',
    'Keep editing retains the draft',
  );
  dialog.dispatchEvent(new win.Event('cancel', { cancelable: true }));
  await until(() => confirmation.open);
  assert.equal(dialog.open, true, 'Escape must wait for confirmation before closing');
  confirmation.querySelector('[data-discard="confirm"]').click();
  await until(() => !document.querySelector('.event-edit-dialog'));
  assert.equal(changes.length, savedChanges, 'discarding must not save');
  assert.equal(recipient.events.length, initialCount);
  document.querySelector('.recipient-history > .recipient-history-heading button').click();
  await until(() => document.querySelector('.event-review'));
  assert.equal(
    document.querySelector('form.event-correction'),
    null,
    'reopening must not restore the discarded draft',
  );
  document.querySelector('.event-edit-dialog .recipient-history-heading button').click();
  await until(() => !document.querySelector('.event-edit-dialog'));
  assert.equal(confirmation.open, false, 'closing without a draft needs no confirmation');
  process.exit(0);
}

if (['missing', 'forbidden', 'retry', 'invalid'].includes(scenario)) {
  await until(() => document.querySelector('.status-page'));
  assert.equal(win.location.pathname, path);
  assert.equal(document.querySelector('.analysis-filters'), null);
  assert.ok(
    !requests.some((url) => url.includes('/results')),
    'do not fetch results before resolving the entry',
  );
  if (scenario !== 'retry') {
    process.exit(0);
  }
  failure = null;
  document.querySelector('.status-page button').click();
}
if (scenario === 'new') {
  await until(() => document.querySelector('.step-navigation'));
  assert.equal(win.location.pathname, '/campaigns/new');
  assert.ok(!requests.some((url) => url.startsWith('/api/campaigns/new')));
  process.exit(0);
}
if (['dashboard', 'directory'].includes(scenario)) {
  await until(() => document.querySelector('a[href="/campaigns/2/results"]'));
  const listed =
    scenario === 'dashboard'
      ? data.campaigns.filter((c) =>
          [...document.querySelectorAll('.dashboard-campaigns a')].some(
            (link) => link.textContent === c.header.name,
          ),
        )
      : data.campaigns;
  assert.ok(listed.some((c) => c.header.status === 'Completed'));
  assert.ok(listed.some((c) => c.header.status !== 'Completed'));
  for (const item of listed) {
    const suffix = item.header.status === 'Completed' ? 'results' : 'delivery';
    const links = [...document.querySelectorAll(`a[href^="/campaigns/${item.header.id}"]`)];
    assert.ok(
      links.some((link) => link.getAttribute('href') === `/campaigns/${item.header.id}/${suffix}`),
    );
    assert.ok(
      links.every((link) => link.getAttribute('href') !== `/campaigns/${item.header.id}`),
      'no duplicate results entry links',
    );
  }
  document.querySelector('a[href="/campaigns/2/results"]').click();
}
if (['active-entry', 'queued-entry'].includes(scenario)) {
  await until(() => win.location.pathname === '/campaigns/2/delivery' && settled());
  assert.equal(win.history.length, 1, 'entry redirect replaces history');
  assert.ok(
    !requests.some((url) => url.includes('/results')),
    'delivery entry does not mount results',
  );
  document.querySelector('.campaign-sections a[href="/campaigns/2/results"]').click();
}
await until(
  () =>
    win.location.pathname === resultsPath &&
    settled() &&
    document.querySelector('.analysis-filters'),
);
if (scenario === 'bookmark') {
  assert.equal(win.location.search, '?source=mailbox');
  assert.equal(win.location.hash, '#history');
}
if (['completed-entry', 'bookmark', 'retry'].includes(scenario)) {
  assert.equal(win.history.length, 1);
}
const form = document.querySelector('.analysis-filters');
const heading = document.querySelector('h1');
const controls = form.closest('.query-controls');
for (const source of ['operator', 'public', scenario === 'bookmark' ? 'legacy' : 'mailbox']) {
  const select = form.querySelector('select');
  select.value = source;
  select.dispatchEvent(new win.Event('change', { bubbles: true }));
  hold = true;
  form.querySelector('button[type="submit"]').click();
  await until(() => release);
  assert.equal(win.location.pathname, resultsPath);
  assert.equal(
    document.querySelector('.analysis-filters'),
    form,
    'filtering must not remount the form',
  );
  assert.equal(
    document.querySelector('h1'),
    heading,
    'filtering must not remount the campaign header',
  );
  assert.ok(!controls.hidden);
  assert.ok(
    regions().every((e) => e.dataset.ready === 'true'),
    'no initial loading during filter transitions',
  );
  assert.equal(document.querySelector('.query-placeholder'), null);
  assert.equal(form.querySelector('button[type="submit"]').getAttribute('aria-busy'), 'true');
  hold = false;
  release();
  release = null;
  await until(settled);
}
process.exit(0);
