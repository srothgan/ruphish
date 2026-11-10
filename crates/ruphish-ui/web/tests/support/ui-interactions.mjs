import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { setImmediate as nextTurn } from 'node:timers/promises';
import { JSDOM } from 'jsdom';
import jsdomUtils from 'jsdom/lib/generated/idl/utils.js';
import whatwgURL from 'whatwg-url';
import { createDemoApi, preferences } from '../../demo-api.mjs';
import { dataset } from '../../../../../assets/website-demo/dataset.mjs';

const scenario = process.argv[2];
const settingsHidden = scenario.startsWith('settings-hidden-');
const recipientEditor = ['recipient-editor', 'recipient-create', 'recipient-duplicate'].includes(
  scenario,
);
const returnOrigin =
  '/campaigns/2/results?source=public&event=link_clicked&from=0&until=1999999999&sort=name&order=desc&offset=1&limit=25';
const path =
  {
    'auth-existing': '/login?next=/',
    'auth-existing-loop': '/login?next=%2Fgroups%2F..%2Flogin',
    'auth-existing-reset': '/login?next=/campaigns',
    'auth-other-tab': '/login?next=/campaigns',
    'auth-login-refresh': '/login?next=/campaigns',
    'auth-login-stale': '/login?next=/campaigns',
    'auth-login-late': '/login?next=/campaigns',
    'auth-login-retry': '/login?next=/campaigns',
    'conflict-draft': '/campaigns/drafts/1/edit',
    'conflict-group': '/groups/1/edit',
    'deleted-group': '/groups/1/edit',
    'conflict-domain': '/domains/1/edit',
    'conflict-webhook': '/webhooks/1/edit',
    'conflict-mailbox': '/reporting-mailboxes/1/edit',
    'delete-conflict': '/groups',
    'delete-conflict-webhook': '/webhooks',
    'delete-conflict-mailbox': '/reporting-mailboxes',
    'delete-conflict-draft': '/campaigns',
    'delete-conflict-draft-detail': '/campaigns/drafts/1',
    'empty-group': '/groups/6',
    'empty-webhook': '/webhooks/1',
    'empty-mailbox': '/reporting-mailboxes/1',
    'empty-directory': '/groups',
    'empty-inventory': '/groups',
    'missing-webhook-directory': '/webhooks',
    'missing-smtp': '/smtp/1',
    'missing-content': '/learning-pages/1',
    'missing-webhook-values': '/webhooks/1',
    'missing-mailbox-values': '/reporting-mailboxes/1',
    'history-domain': '/domains/1',
    'detail-group': '/groups/1',
    'detail-template': '/templates/1',
    'detail-page': '/pages/1',
    'detail-learning': '/learning-pages/1',
    'detail-smtp': '/smtp/1',
    'detail-domain': '/domains/1',
    'detail-webhook': '/webhooks/1',
    'detail-mailbox': '/reporting-mailboxes/1',
    'detail-draft': '/campaigns/drafts/1',
    'domain-attribution': '/domains/1',
    'shared-attribution': '/groups/1/edit',
    'unknown-attribution': '/groups/1/edit',
    'form-import': '/groups?import=1',
    'form-webhook': '/webhooks/new',
    'integration-readonly-webhook': '/webhooks/new',
    'integration-readonly-mailbox': '/reporting-mailboxes/new',
    'form-mailbox': '/reporting-mailboxes/new',
    'form-filters': returnOrigin,
    'form-group': '/groups/new',
    'recipient-editor': '/groups/1/edit',
    'recipient-create': '/groups/new',
    'recipient-duplicate': '/groups/new?duplicate=1',
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
    'settings-hidden-validity': '/settings',
    'settings-hidden-recheck': '/settings',
    'form-campaign': '/campaigns/new',
    'campaign-wizard': '/campaigns/new',
    'form-draft': '/campaigns/drafts/1/edit',
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
    'sort-directory': '/campaigns',
    invalid: '/campaigns/not-an-id',
    new: '/campaigns/new',
  }[scenario] ?? '/campaigns/2';
const dom = new JSDOM('<!doctype html><html><body></body></html>', {
  url: `https://admin.example.test${path}`,
  pretendToBeVisual: true,
});
const win = dom.window;
// JSDOM cannot load a new document. Observe the actual Location boundary instead.
const authNavigations = [];
if (scenario.startsWith('auth-')) {
  const location = jsdomUtils.implForWrapper(win.location);
  location._locationObjectNavigate = (url, options = {}) => {
    authNavigations.push({
      url: whatwgURL.serializeURL(url),
      replacement: options.replacement === true,
    });
  };
  location.reload = () => authNavigations.push({ reload: true });
}
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
if (
  scenario.startsWith('auth-') ||
  settingsHidden ||
  scenario.startsWith('form-') ||
  scenario.startsWith('detail-') ||
  scenario.startsWith('empty-') ||
  scenario.startsWith('missing-') ||
  scenario === 'history-domain' ||
  scenario.startsWith('integration-') ||
  scenario === 'campaign-wizard' ||
  scenario === 'sort-directory' ||
  recipientEditor ||
  scenario.endsWith('-attribution') ||
  scenario.startsWith('conflict-') ||
  scenario === 'deleted-group' ||
  scenario.startsWith('delete-conflict')
) {
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
if (recipientEditor) {
  data.groups[0].targets = ['z', 'a', 'b'].map((name) => ({
    email: `${name}@example.test`,
    first_name: name,
    last_name: '',
    position: '',
  }));
}
if (scenario === 'empty-inventory') {
  data.groups = [];
}
if (scenario === 'empty-webhook') {
  data.webhooks[0].history = [];
}
if (scenario === 'empty-mailbox') {
  data['reporting-mailboxes'][0].history = [];
}
if (scenario === 'missing-smtp') {
  Object.assign(data.smtp[0], {
    username: '   ',
    headers: [],
    has_password: false,
    ignore_cert_errors: false,
  });
}
if (scenario === 'missing-content') {
  data['learning-pages'][0].html = '';
}
if (scenario === 'missing-webhook-values') {
  Object.assign(data.webhooks[0].history[0], { status: null, event_id: null, attempt: null });
}
if (scenario === 'missing-mailbox-values') {
  Object.assign(data['reporting-mailboxes'][0], {
    username: '   ',
    code: '',
    last_success: 0,
    enabled: false,
  });
  Object.assign(data['reporting-mailboxes'][0].history[0], { matched: 0, rejected: 0 });
}
if (scenario === 'history-domain') {
  data.domains[0].history = [
    {
      id: 1,
      state: 'passed',
      code: 'domain_verified',
      started_at: now - 10,
      finished_at: now - 10,
      scheduled_at: now - 10,
    },
  ];
}

if (scenario.endsWith('-attribution')) {
  Object.assign(scenario === 'domain-attribution' ? data.domains[0] : data.groups[0], {
    created_at: scenario === 'unknown-attribution' ? null : '2026-10-24T12:00:00Z',
    created_by: 100,
    edited_by: scenario === 'unknown-attribution' ? null : data.users[1].id,
    edited_at: scenario === 'unknown-attribution' ? null : '2026-10-25T12:00:00Z',
  });
}

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
if (scenario === 'sort-directory') {
  data.drafts.forEach((draft, index) => {
    draft.configuration.name = `Zulu draft ${index}`;
  });
  data.campaigns = Array.from({ length: 55 }, (_, i) => ({
    ...campaign,
    header: {
      ...campaign.header,
      id: i + 1,
      name: `Campaign ${String(i + 1).padStart(2, '0')}`,
      created_at: new Date((now + i) * 1000).toISOString(),
    },
  }));
}
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
let authSignedIn = scenario.startsWith('auth-existing');
let authLoginRelease;
let authLoginStatus = 200;
let authSessionFinished = 0;
let authLoginCount = 0;
let releaseValidation;
const featureOverrides = { webhooks_enabled: true, reporting_mailboxes_enabled: true };
let failSave = false;
const writes = [];
const recoveryPath = {
  'conflict-draft': '/api/campaign-drafts/1',
  'conflict-group': '/api/groups/1',
  'deleted-group': '/api/groups/1',
  'conflict-domain': '/api/domains/1',
  'conflict-webhook': '/api/webhooks/1',
  'conflict-mailbox': '/api/reporting-mailboxes/1',
}[scenario];
let recoveryAttempted = false;
let recoveryAccepted = false;
let recoveryRevision;
let holdRecoveryRead = false;
let releaseRecoveryRead;

globalThis.fetch = async (request, options) => {
  const url = new URL(request.url);
  requests.push(url.pathname + url.search);
  if (settingsHidden && url.pathname === '/api/settings' && request.method === 'GET') {
    const response = await api.execute(request, options);
    const value = await response.json();
    value.policy.verification_enabled = false;
    return new Response(JSON.stringify(value), {
      status: response.status,
      headers: response.headers,
    });
  }
  if (
    recipientEditor &&
    /^\/api\/groups\/(?:1)?$/.test(url.pathname) &&
    ['PUT', 'POST'].includes(request.method)
  ) {
    const body = await request.clone().json();
    writes.push(body);
    if (failSave) {
      return new Response(
        JSON.stringify({
          success: false,
          code: 'VALIDATION_FAILED',
          validation: {
            issues: [
              {
                field: `targets.${body.targets.findIndex((row) => row.email === 'd@example.test')}.email`,
                code: 'invalid',
              },
            ],
          },
        }),
        { status: 400 },
      );
    }
    // The public demo intentionally discards resource writes. Editor regressions need
    // committed fixture state, while retaining its request/revision checks and read contract.
    const accepted = await api.execute(request, options);
    if (!accepted.ok) {
      return accepted;
    }
    const creating = request.method === 'POST';
    const previous = creating
      ? null
      : data.groups.find((row) => row.id === Number(url.pathname.split('/').at(-1)));
    assert.ok(creating || previous, 'an accepted update must have an existing group');
    const savedAt = new Date().toISOString();
    const saved = {
      ...structuredClone(previous ?? data.groups[0]),
      id: creating ? Math.max(0, ...data.groups.map((row) => row.id)) + 1 : previous.id,
      name: body.name,
      targets: structuredClone(body.targets),
      revision: creating ? 1 : previous.revision + 1,
      modified_date: savedAt,
      created_at: creating ? savedAt : previous.created_at,
      created_by: creating ? 1 : previous.created_by,
      edited_at: savedAt,
      edited_by: 1,
    };
    if (creating) {
      data.groups.push(saved);
    } else {
      data.groups[data.groups.indexOf(previous)] = saved;
    }
    const response = await api.execute(new Request(new URL(`/api/groups/${saved.id}`, url)));
    assert.equal(response.status, 200, 'the saved group must be readable');
    return new Response(await response.text(), {
      status: creating ? 201 : 200,
      headers: response.headers,
    });
  }
  if (scenario.startsWith('auth-') && url.pathname === '/auth/login') {
    authLoginCount += 1;
    const fields = new URLSearchParams(await request.text());
    assert.equal(fields.get('next'), '/campaigns');
    assert.equal(fields.get('username'), 'demo');
    if (authLoginStatus !== 200) {
      return new Response(JSON.stringify({ success: false, code: 'AUTH_INVALID' }), {
        status: authLoginStatus,
      });
    }
    authSignedIn = true;
    await new Promise((resolve) => {
      authLoginRelease = resolve;
    });
    return new Response(JSON.stringify({ success: true, data: '/campaigns' }));
  }
  if (recoveryPath === url.pathname) {
    if (request.method === 'PUT') {
      const body = await request.clone().json();
      writes.push({ body, revision: request.headers.get('if-match') ?? String(body.revision) });
      if (!recoveryAttempted || !recoveryAccepted) {
        recoveryAttempted = true;
        return new Response(
          JSON.stringify({
            success: false,
            code: scenario === 'deleted-group' ? 'RESOURCE_NOT_FOUND' : 'RESOURCE_CHANGED',
          }),
          { status: scenario === 'deleted-group' ? 404 : 412 },
        );
      }
      assert.equal(writes.at(-1).revision.replaceAll('"', ''), String(recoveryRevision + 1));
      const response = await api.execute(new Request(request.url));
      const saved = await response.json();
      if (scenario === 'conflict-draft') {
        saved.configuration = body.configuration;
        saved.step = body.step;
      } else {
        saved.name = body.name;
      }
      saved.revision = recoveryRevision + 2;
      return new Response(JSON.stringify(saved), { headers: { etag: `"${saved.revision}"` } });
    }
    if (request.method === 'GET') {
      if (recoveryAttempted && holdRecoveryRead) {
        await new Promise((resolve) => {
          releaseRecoveryRead = resolve;
        });
      }
      const response = await api.execute(request, options);
      const saved = await response.json();
      recoveryRevision ??= saved.revision;
      if (recoveryAttempted) {
        if (scenario === 'conflict-draft') {
          saved.configuration.name = 'Colleague saved version';
        } else {
          saved.name = 'Colleague saved version';
        }
        saved.revision = recoveryRevision + 1;
      }
      return new Response(JSON.stringify(saved), { headers: { etag: `"${saved.revision}"` } });
    }
  }
  if (scenario.startsWith('delete-conflict') && request.method === 'DELETE') {
    writes.push({
      path: url.pathname,
      revision: request.headers.get('if-match') ?? url.searchParams.get('revision'),
    });
    return new Response(JSON.stringify({ success: false, code: 'RESOURCE_CHANGED' }), {
      status: 412,
    });
  }

  if (
    (scenario.startsWith('form-') || scenario === 'campaign-wizard') &&
    ['POST', 'PUT'].includes(request.method)
  ) {
    writes.push({
      path: url.pathname,
      body: await request.clone().json(),
      key: request.headers.get('idempotency-key'),
    });
    if (failSave) {
      if (scenario === 'campaign-wizard' && url.pathname === '/api/campaign-drafts') {
        const accepted = await api.execute(request.clone(), options);
        assert.equal(accepted.status, 201, 'creation persisted before losing its acknowledgement');
      }
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
    const authSnapshot = authSignedIn;
    if (holdSession) {
      await new Promise((resolve) => {
        releaseSession = resolve;
      });
    }
    const response = await api.execute(request, options);
    const value = await response.json();
    value.features = { ...value.features, ...featureOverrides };
    if (
      ['form-webhook', 'form-mailbox'].includes(scenario) ||
      scenario.startsWith('integration-')
    ) {
      value.user.role = { slug: 'user', name: 'User', description: 'Regular user' };
      value.user.permissions = scenario.startsWith('integration-readonly')
        ? ['view_objects']
        : ['view_objects', 'modify_objects'];
      value.actor = null;
    }
    if (sessionExpired) {
      value.user = null;
    }
    if (scenario.startsWith('auth-')) {
      if (scenario === 'auth-existing-reset') {
        value.user.password_change_required = true;
      }
      if (
        !(['auth-login-stale', 'auth-login-late'].includes(scenario) ? authSnapshot : authSignedIn)
      ) {
        value.user = null;
      }
      authSessionFinished += 1;
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
const demo = html.includes('<title>Ruphish Demo</title>');
if (scenario.startsWith('auth-')) {
  assert.equal(demo, false, 'auth regressions need the normal WASM bundle');
}
if (scenario === 'campaign-wizard' && demo) {
  win.sessionStorage.setItem('ruphish-demo-session-v1', '1');
  win.sessionStorage.setItem('ruphish-demo-time-v1', String(now));
  globalThis.fetch = win.fetch = () => {
    throw new Error('The demo must not contact a server');
  };
}
const script = html.match(/import init, \* as bindings from '\/(.*?)'/)[1];
const { initSync } = await import(new URL(script, dist));
initSync({ module: readFileSync(new URL(script.replace(/\.js$/, '_bg.wasm'), dist)) });

async function until(condition) {
  const deadline = performance.now() + 15000;
  while (!condition()) {
    if (
      (scenario.startsWith('auth-') ||
        settingsHidden ||
        scenario === 'campaign-wizard' ||
        scenario === 'sort-directory' ||
        recipientEditor ||
        recoveryPath ||
        scenario.startsWith('delete-conflict') ||
        scenario.startsWith('empty-') ||
        scenario.startsWith('missing-') ||
        scenario.startsWith('detail-') ||
        scenario === 'history-domain') &&
      performance.now() > deadline
    ) {
      const current = win.document.querySelector('[aria-current="step"]')?.textContent;
      const feedback = [
        ...win.document.querySelectorAll('.form-message.has-content, .demo-toast.has-content'),
      ]
        .map((node) => node.textContent)
        .join(' | ');
      assert.fail(
        `UI scenario ${scenario} did not settle at ${win.location.pathname}, ${current}: ${condition}. ${feedback}`,
      );
    }
    await nextTurn();
  }
}
const document = win.document;
const regions = () => [...document.querySelectorAll('.query-region')];
const settled = () =>
  regions().length > 0 &&
  regions().every((e) => e.dataset.ready === 'true' && e.getAttribute('aria-busy') === 'false');
const resultsPath = '/campaigns/2/results';

if (scenario.startsWith('auth-')) {
  if (scenario.startsWith('auth-existing')) {
    const destination = scenario === 'auth-existing-reset' ? '/reset_password' : '/';
    await until(
      () => win.location.pathname === destination && document.querySelector('.admin-shell'),
    );
    assert.doesNotMatch(document.querySelector('.admin-content').textContent, /404/);
    if (scenario === 'auth-existing-reset') {
      assert.ok(document.querySelector('.account-form'));
    }
    assert.equal(authNavigations.length, 0);
    process.exit(0);
  }
  await until(() => document.querySelector('.login-form form'));
  if (scenario === 'auth-other-tab') {
    authSignedIn = true;
    win.dispatchEvent(new win.Event('focus'));
    await until(() => authNavigations.length > 0);
  } else {
    const form = document.querySelector('.login-form form');
    const submit = () => {
      for (const [name, value] of [
        ['username', 'demo'],
        ['password', 'demodemo'],
      ]) {
        const input = form.elements.namedItem(name);
        input.value = value;
        input.dispatchEvent(new win.Event('input', { bubbles: true }));
      }
      form.dispatchEvent(new win.Event('submit', { bubbles: true, cancelable: true }));
    };
    if (scenario === 'auth-login-retry') {
      for (const status of [401, 503]) {
        authLoginStatus = status;
        const count = authLoginCount;
        submit();
        await until(
          () => authLoginCount > count && !form.querySelector('button[type="submit"]').disabled,
        );
        assert.equal(authNavigations.length, 0);
        const reads = authSessionFinished;
        win.dispatchEvent(new win.Event('focus'));
        await until(() => authSessionFinished > reads);
      }
      authLoginStatus = 200;
    } else {
      holdSession = true;
      win.dispatchEvent(new win.Event('focus'));
      await until(() => releaseSession);
    }
    submit();
    await until(() => authLoginRelease);
    submit();
    const count = authLoginCount;
    win.dispatchEvent(new win.Event('focus'));
    if (releaseSession && scenario !== 'auth-login-late') {
      const reads = authSessionFinished;
      holdSession = false;
      releaseSession();
      await until(() => authSessionFinished > reads);
      for (let i = 0; i < 20; i++) {
        await nextTurn();
      }
      assert.equal(authNavigations.length, 0, 'refresh must not preempt login navigation');
    }
    authLoginRelease();
    await until(() => authNavigations.length > 0);
    if (scenario === 'auth-login-late') {
      const reads = authSessionFinished;
      holdSession = false;
      releaseSession();
      await until(() => authSessionFinished > reads);
      for (let i = 0; i < 20; i++) {
        await nextTurn();
      }
    }
    assert.equal(authLoginCount, count, 'duplicate submit must not send another login');
  }
  assert.deepEqual(authNavigations, [
    { url: 'https://admin.example.test/campaigns', replacement: true },
  ]);
  assert.equal(
    document.querySelector('.admin-shell'),
    null,
    'old identity is not displayed during navigation',
  );
  process.exit(0);
}

if (recoveryPath) {
  const button = (name) =>
    [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === name);
  const saveLabel = 'Save changes';
  const nameInput = () => document.querySelector('.admin-content input[type="text"]');
  await until(() => nameInput() && button(saveLabel) && !button(saveLabel).disabled);
  const setName = (name) => {
    const input = nameInput();
    input.value = name;
    input.dispatchEvent(new win.Event('input', { bubbles: true }));
  };
  setName('My unsaved version');
  button(saveLabel).click();
  await until(() =>
    document
      .querySelector('code')
      ?.textContent.includes(
        scenario === 'deleted-group' ? 'RESOURCE_NOT_FOUND' : 'RESOURCE_CHANGED',
      ),
  );
  assert.equal(nameInput().value, 'My unsaved version');
  const reads = requests.filter((p) => p === recoveryPath).length;
  if (scenario === 'deleted-group') {
    assert.ok(document.body.textContent.includes('input is still here'));
    assert.ok(!button('Discard my changes and reload'));
    assert.equal(writes.length, 1);
    process.exit(0);
  }
  const review = [...document.querySelectorAll('a')].find(
    (a) => a.textContent.trim() === 'Review saved version in a new tab',
  );
  assert.equal(
    review.getAttribute('href'),
    recoveryPath.slice(4).replace('/campaign-drafts/', '/campaigns/drafts/'),
  );
  assert.equal(review.target, '_blank');
  button('Discard my changes and reload').click();
  await until(() => document.querySelector('dialog[open]'));
  document.querySelector('dialog[open] button').click();
  await until(() => !document.querySelector('dialog[open]'));
  assert.equal(nameInput().value, 'My unsaved version');
  assert.equal(
    requests.filter((p) => p === recoveryPath).length,
    reads,
    'cancel does not fetch or advance the revision',
  );
  button(saveLabel).click();
  await until(() => writes.length === 2 && !button(saveLabel).disabled);
  assert.equal(
    writes[0].revision,
    writes[1].revision,
    'retry never silently uses a newer revision',
  );
  button('Discard my changes and reload').click();
  await until(() => document.querySelector('dialog[open]'));
  holdRecoveryRead = true;
  [...document.querySelectorAll('dialog[open] button')]
    .find((b) => b.textContent.trim() === 'Discard my changes and reload')
    .click();
  await until(() => releaseRecoveryRead);
  assert.ok(
    nameInput().matches(':disabled') || nameInput().closest('[hidden]'),
    'reload blocks new edits until the reviewed revision arrives',
  );
  assert.ok(button(saveLabel).disabled, 'cannot submit against the old revision during reload');
  holdRecoveryRead = false;
  releaseRecoveryRead();
  await until(() => nameInput()?.value === 'Colleague saved version');
  assert.ok(!document.body.textContent.includes('RESOURCE_CHANGED'));
  recoveryAccepted = true;
  setName('Reviewed and reconciled');
  button(saveLabel).click();
  await until(() => writes.length === 3);
  await until(() => !button(saveLabel) || !button(saveLabel).disabled);
  process.exit(0);
}
if (scenario.startsWith('delete-conflict')) {
  const selector = scenario.endsWith('-detail') ? '.admin-content button' : 'tbody button';
  await until(() =>
    [...document.querySelectorAll(selector)].some((b) => b.textContent.trim() === 'Delete'),
  );
  const container =
    scenario === 'delete-conflict-draft'
      ? document.querySelector('tbody a[href^="/campaigns/drafts/"]').closest('tr')
      : document;
  const buttons = [
    ...container.querySelectorAll(scenario === 'delete-conflict-draft' ? 'button' : selector),
  ];
  buttons.find((b) => b.textContent.trim() === 'Delete').click();
  await until(() => document.querySelector('dialog[open]'));
  const confirm = [...document.querySelectorAll('dialog[open] button')].find(
    (b) => b.textContent.trim() === 'Delete',
  );
  confirm.click();
  await until(
    () => document.querySelector('dialog[open] code')?.textContent === 'RESOURCE_CHANGED',
  );
  assert.match(
    writes[0].revision,
    scenario.includes('draft') ? /^[1-9][0-9]*$/ : /^"[1-9][0-9]*"$/,
  );
  if (scenario.includes('draft')) {
    assert.ok(writes[0].path.startsWith('/api/campaign-drafts/'));
  }
  assert.equal(confirm.disabled, true);
  assert.ok(document.querySelector('dialog[open] a[target="_blank"]'));
  assert.ok(document.querySelector('dialog[open]').textContent.includes('refresh this page'));
  confirm.click();
  await nextTurn();
  assert.equal(writes.length, 1);
  [...document.querySelectorAll('dialog[open] button')]
    .find((b) => b.textContent.trim() === 'Cancel')
    .click();
  await until(() => !document.querySelector('dialog[open]'));
  if (!scenario.endsWith('-detail')) {
    assert.ok(document.querySelector('tbody tr'));
  }
  process.exit(0);
}

const visibleTables = (root = document) =>
  [...root.querySelectorAll('table')].filter((table) => !table.closest('[hidden]'));
const detailValue = (label) =>
  [...document.querySelectorAll('.detail-card dt')]
    .find((node) => node.textContent.trim() === label)
    ?.nextElementSibling?.textContent.trim();
if (
  scenario.startsWith('empty-') ||
  scenario.startsWith('missing-') ||
  scenario === 'history-domain'
) {
  await until(settled);
  if (scenario === 'empty-directory') {
    const search = document.querySelector('.management-toolbar input');
    assert.ok(visibleTables().length > 0);
    search.value = 'No such recipient group';
    search.dispatchEvent(new win.Event('input', { bubbles: true }));
    await until(() => visibleTables().length === 0);
    assert.match(document.querySelector('.table-empty').textContent, /No matching results/);
    assert.match(document.querySelector('.table-empty').textContent, /Try another search/);
    search.value = '';
    search.dispatchEvent(new win.Event('input', { bubbles: true }));
    await until(() => visibleTables().length > 0);
    assert.ok(document.querySelector('.table-empty').hidden);
  } else if (scenario === 'empty-inventory') {
    assert.equal(visibleTables().length, 0);
    assert.match(document.querySelector('.table-empty').textContent, /No recipient groups yet/);
    assert.doesNotMatch(document.querySelector('.table-empty').textContent, /No matching results/);
  } else if (scenario.startsWith('empty-')) {
    assert.equal(visibleTables().length, 0, 'an empty collection exposes no sorting headers');
    assert.match(
      document.querySelector('.detail-card').textContent,
      scenario === 'empty-group' ? /No recipients added/ : /No processing history recorded yet/,
    );
    if (scenario === 'empty-group') {
      assert.equal(detailValue('Recipients'), '0');
    }
  } else if (scenario === 'missing-webhook-directory') {
    const rows = [...visibleTables()[0].querySelectorAll('tbody tr')];
    assert.ok(rows.length > 0);
    for (const row of rows) {
      assert.equal(row.children[3].textContent.trim(), 'Not applicable');
    }
  } else if (scenario === 'missing-smtp') {
    assert.equal(detailValue('Username'), 'Not set');
    assert.equal(detailValue('Password'), 'Not configured');
    assert.equal(detailValue('Ignore certificate errors'), 'No');
    assert.match(
      document.querySelector('.detail-card').textContent,
      /No additional headers configured/,
    );
  } else if (scenario === 'missing-content') {
    assert.match(document.querySelector('.detail-card').textContent, /No content available/);
    assert.equal(document.querySelector('.saved-content-preview'), null);
  } else if (scenario === 'missing-webhook-values') {
    const values = [...visibleTables()[0].querySelectorAll('tbody td')].map((node) =>
      node.textContent.trim(),
    );
    assert.deepEqual(values.slice(-3), ['No HTTP response', 'Not applicable', 'Not applicable']);
  } else if (scenario === 'missing-mailbox-values') {
    assert.equal(detailValue('Account'), 'Not set');
    assert.equal(detailValue('Last outcome'), 'Not recorded');
    assert.equal(detailValue('Last successful check'), 'Not recorded');
    assert.equal(detailValue('Next check'), 'Disabled');
    const values = [...visibleTables()[0].querySelectorAll('tbody td')].map((node) =>
      node.textContent.trim(),
    );
    assert.deepEqual(values.slice(-2), ['0', '0']);
  } else {
    const history = document.querySelector('.domain-history');
    assert.equal(visibleTables(history).length, 1);
    assert.equal(history.querySelectorAll('tbody tr').length, 1);
    assert.ok(history.querySelector('.table-empty').hidden);
  }
  process.exit(0);
}

if (scenario.startsWith('detail-')) {
  await until(() =>
    document
      .querySelector('section[aria-label="Record information"] dd')
      ?.textContent.includes('2026'),
  );
  const card = document.querySelector('.detail-card');
  assert.ok(card, 'record values have a shared white detail surface');
  assert.equal(document.querySelectorAll('.detail-card').length, 1);
  assert.equal(card.querySelectorAll('.card').length, 0, 'sections are not nested cards');
  assert.equal(document.querySelector('.management-heading p'), null, 'no record-data subtitle');
  assert.ok(!card.contains(document.querySelector('h1')), 'the title remains outside');
  for (const grid of document.querySelectorAll('.resource-facts')) {
    assert.ok(card.contains(grid), 'every read-only field group belongs to the record card');
    for (const value of grid.querySelectorAll('dd')) {
      assert.equal(value.previousElementSibling?.tagName, 'DT', 'values retain their label');
    }
  }
  const record = card.querySelector('section[aria-label="Record information"]');
  assert.deepEqual(
    [...record.querySelectorAll('dt')].map((n) => n.textContent.trim()),
    ['Created at', 'Created by', 'Last edited at', 'Last edited by'],
  );
  assert.ok(!card.querySelector('.resource-actions'), 'record actions remain outside');
  if (scenario === 'detail-domain') {
    assert.ok(data.domains[0].checked_at > 0, 'a past check does not guarantee retained history');
    const history = card.querySelector('.domain-history');
    assert.ok(
      history.querySelector('.management-table-wrap').hidden,
      'empty history hides its table and sorting controls',
    );
    assert.match(history.textContent, /No verification history available/);
  }
  process.exit(0);
}

if (scenario.endsWith('-attribution')) {
  await until(() =>
    document
      .querySelector('section[aria-label="Record information"]')
      ?.textContent.includes('Morgan Brooks'),
  );
  const metadata = document.querySelector('section[aria-label="Record information"]');
  assert.match(metadata.textContent, /Created by/);
  assert.match(metadata.textContent, /Morgan Brooks.*deleted/i);
  assert.equal(metadata.querySelectorAll('input,select,textarea,[contenteditable]').length, 0);
  const values = [...metadata.querySelectorAll('dd')].map((node) => node.textContent.trim());
  if (scenario === 'unknown-attribution') {
    assert.equal(values[0], 'Unknown');
    assert.deepEqual(values.slice(2), ['Unknown', 'Unknown']);
  } else {
    assert.equal(values[3], data.users[1].username);
    assert.match(values[0], /2026/);
    assert.match(values[2], /2026/);
  }
  assert.equal(
    requests.filter(
      (path) => path === (scenario === 'domain-attribution' ? '/api/domains/1' : '/api/groups/1'),
    ).length,
    1,
  );
  assert.ok(!requests.some((path) => path.startsWith('/api/authors')));
  if (scenario === 'domain-attribution') {
    process.exit(0);
  }
  const before = metadata.textContent;
  const name = document.querySelector('input');
  name.value = 'Unsaved shared edit';
  name.dispatchEvent(new win.Event('input', { bubbles: true }));
  await nextTurn();
  assert.equal(metadata.textContent, before, 'typing is not a confirmed edit');
  process.exit(0);
}

if (scenario === 'campaign-wizard') {
  const button = (text) =>
    [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === text);
  const step = () =>
    [...document.querySelectorAll('.step-navigation button')].findIndex(
      (b) => b.getAttribute('aria-current') === 'step',
    );
  const field = (text) => {
    const label = [...document.querySelectorAll('label')].find(
      (l) =>
        [...l.childNodes]
          .filter((n) => n.nodeType === win.Node.TEXT_NODE)
          .map((n) => n.textContent)
          .join('')
          .trim() === text,
    );
    assert.ok(label, `field label: ${text}`);
    return document.getElementById(label.htmlFor) ?? label.querySelector('input,select');
  };
  const set = (control, value) => {
    assert.ok(control);
    control.value = value;
    control.dispatchEvent(
      new win.Event(control.tagName === 'SELECT' ? 'change' : 'input', { bubbles: true }),
    );
  };
  const advance = async (expected) => {
    // Separate user actions so reactive disabled states and page-default requests can settle.
    await nextTurn();
    await until(() => button('Continue') && !button('Continue').disabled);
    button('Continue').click();
    await until(() => step() === expected && !button('Continue').disabled);
  };
  const link = (href) => {
    const anchor = document.querySelector(`a[href="${href}"]`);
    assert.ok(anchor, `navigation link: ${href}`);
    anchor.click();
  };
  await until(() => settled() && button('Continue') && !button('Continue').disabled);
  button('Continue').click();
  await nextTurn();
  assert.equal(step(), 0, 'invalid audience stays on its phase');
  assert.ok(document.querySelector('[aria-invalid="true"]'));
  assert.equal(win.location.pathname, '/campaigns/new');
  set(field('Campaign name'), 'Temporary wizard campaign');
  document.querySelector('section:not([hidden]) input[type="checkbox"]').click();
  if (!demo) {
    set(field('Campaign name'), 'Before lost response');
    failSave = true;
    button('Continue').click();
    await until(
      () =>
        document.querySelector('.form-message.error.has-content') && !button('Continue').disabled,
    );
    assert.equal(step(), 0, 'a rejected save must not advance');
    assert.equal(field('Campaign name').value, 'Before lost response');
    set(field('Campaign name'), 'Temporary wizard campaign');
    failSave = false;
  }
  await advance(1);
  if (!demo) {
    const creates = writes.filter((w) => w.path === '/api/campaign-drafts');
    assert.equal(creates.length, 2);
    assert.ok(creates[0].key);
    assert.deepEqual(creates[1], creates[0], 'lost response retries the original key and input');
  }
  const editPath = win.location.pathname;
  assert.match(editPath, /^\/campaigns\/drafts\/[1-9][0-9]*\/edit$/);
  assert.notEqual(editPath, '/campaigns/drafts/1/edit', 'new draft keeps its own ID');
  assert.equal(field('Campaign name').value, 'Temporary wizard campaign');
  assert.ok(!document.querySelector('.demo-toast.has-content'), 'Continue needs no save toast');
  button('Continue').click();
  await nextTurn();
  assert.equal(step(), 1, 'invalid content stays on its phase');
  for (const name of ['Email template', 'Sending profile', 'Landing page']) {
    set(field(name), '1');
  }
  const origin = field('Campaign origin');
  set(origin, origin.options[1].value);
  await advance(2);
  button('Back').click();
  await until(() => step() === 1 && !button('Continue').disabled);
  assert.equal(field('Email template').value, '1');
  document.querySelectorAll('.step-navigation button')[2].click();
  await until(() => step() === 2 && !button('Continue').disabled);
  button('Save changes').click();
  const detailPath = editPath.replace(/\/edit$/, '');
  await until(() => win.location.pathname === detailPath && settled());
  assert.ok(document.body.textContent.includes('Temporary wizard campaign'));
  if (demo) {
    assert.match(document.querySelector('.demo-toast').textContent, /Draft kept temporarily/);
    button('Dismiss').click();
  }
  link('/campaigns');
  await until(
    () =>
      win.location.pathname === '/campaigns' && document.querySelector(`a[href="${detailPath}"]`),
  );
  assert.ok(document.body.textContent.includes('Temporary wizard campaign'));
  link(detailPath);
  await until(() => document.querySelector(`a[href="${editPath}"]`));
  link(editPath);
  await until(() => step() === 2 && settled());
  assert.equal(field('Campaign name').value, 'Temporary wizard campaign');
  assert.equal(field('Email template').value, '1');
  await advance(3);
  const launchLabel = demo ? 'Simulate launch' : 'Launch campaign';
  await until(() => button(launchLabel) && !button(launchLabel).disabled);
  const reviewCard = document.querySelector('.management-form.detail-card');
  assert.ok(reviewCard);
  assert.equal(reviewCard.querySelectorAll('.card').length, 0);
  assert.ok(reviewCard.contains(document.querySelector('iframe.saved-content-preview')));
  assert.ok(!reviewCard.contains(button(launchLabel)));
  assert.ok(!reviewCard.contains(button('Send test email')));
  assert.ok(reviewCard.querySelector('section[aria-label="Record information"]'));
  assert.equal(document.querySelector('.management-heading p'), null);

  assert.ok(!document.querySelector('.demo-toast.has-content'));
  if (demo) {
    const template = dataset(now).templates.find((row) => row.id === 1);
    const preview = document.querySelector('iframe.saved-content-preview');
    assert.ok(preview.srcdoc.endsWith(template.html), 'preview displays the selected email HTML');
    assert.equal(preview.getAttribute('sandbox'), '');
    assert.equal(preview.getAttribute('referrerpolicy'), 'no-referrer');
    assert.ok(document.body.textContent.includes(template.subject));
    assert.ok(document.querySelector('details pre').textContent.includes(template.text));
    assert.ok(document.body.textContent.includes('without filling in variables'));
    const languages = win.navigator.languages;
    Object.defineProperty(win.navigator, 'languages', { value: ['de-DE'], configurable: true });
    win.dispatchEvent(new win.Event('focus'));
    await until(() => button('Kampagnenstart simulieren'));
    assert.ok(document.body.textContent.includes('ohne Variablen auszufüllen'));
    assert.equal(preview.isConnected, true, 'language refresh preserves the email preview');
    Object.defineProperty(win.navigator, 'languages', { value: languages, configurable: true });
    win.dispatchEvent(new win.Event('focus'));
    await until(() => button(launchLabel));
    set(field('Test recipient email'), 'nobody@example.test');
    await nextTurn();
    await until(() => !button('Send test email').disabled);
    button('Send test email').click();
    await until(() =>
      document.body.textContent.includes('Test email simulated. No email was sent.'),
    );
    assert.ok(!document.body.textContent.includes('Test accepted by SMTP server'));
  }
  await nextTurn();
  await until(() => !button(launchLabel).disabled);
  button(launchLabel).click();
  await until(() => win.location.pathname === '/campaigns/3/delivery' && settled());
  if (demo) {
    assert.match(document.querySelector('.demo-toast').textContent, /showing a sample campaign/);
    assert.deepEqual(Object.keys(win.sessionStorage).sort(), [
      'ruphish-demo-session-v1',
      'ruphish-demo-time-v1',
    ]);
    assert.equal(win.localStorage.length, 0);
  }
  win.history.pushState(null, '', editPath);
  win.dispatchEvent(new win.PopStateEvent('popstate'));
  await until(() => win.location.pathname === '/campaigns/3/delivery' && settled());
  link('/campaigns');
  await until(() => win.location.pathname === '/campaigns' && settled());
  link('/campaigns/new');
  await until(() => button('Save as draft') && settled());
  button('Save as draft').click();
  await until(() => /^\/campaigns\/drafts\/[^/]+$/.test(win.location.pathname) && settled());
  const incompletePath = win.location.pathname;
  assert.notEqual(incompletePath, detailPath, 'separate creations must not replace each other');
  if (demo) {
    assert.match(document.querySelector('.demo-toast').textContent, /Draft kept temporarily/);
  }
  button('Delete').click();
  await until(() => document.querySelector('dialog[open]'));
  [...document.querySelectorAll('dialog button')]
    .find((b) => b.textContent.trim() === 'Delete')
    .click();
  await until(() => win.location.pathname === '/campaigns' && settled());
  assert.ok(!document.querySelector(`a[href="${incompletePath}"]`));
  if (demo) {
    assert.match(document.querySelector('.demo-toast').textContent, /Draft removed from this demo/);
  }
  process.exit(0);
}

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

if (scenario.startsWith('integration-readonly')) {
  await until(() => document.querySelector('form button[type="submit"]'));
  assert.ok(document.querySelector('form button[type="submit"]').disabled);
  assert.ok(document.querySelector('a[href="/webhooks"]'));
  assert.ok(document.querySelector('a[href="/reporting-mailboxes"]'));
  assert.equal(document.querySelector('a[href="/settings"]'), null);
  process.exit(0);
}

if (recipientEditor) {
  const originalGroups = structuredClone(data.groups);
  const editing = scenario === 'recipient-editor';
  const populated = scenario !== 'recipient-create';
  const rows = () => [...document.querySelectorAll('.recipient-table tbody tr')];
  const email = (row) => row.querySelector('input');
  const values = () => rows().map((row) => email(row).value);
  const assertRows = (expected) => {
    assert.equal(rows().length, expected.length);
    expected.forEach((row, index) => assert.equal(rows()[index], row));
  };
  const add = () =>
    [...document.querySelectorAll('button')].find(
      (button) => button.textContent.trim() === 'Add recipient',
    );
  const enter = async (input, value) => {
    input.focus();
    input.value = value;
    input.dispatchEvent(new win.Event('input', { bubbles: true }));
    await nextTurn();
  };
  await until(() => settled() && add() && !add().disabled && rows().length === (populated ? 3 : 0));
  const table = document.querySelector('.recipient-table');
  assert.equal(table.querySelectorAll('thead button, th[aria-sort]').length, 0);
  const initial = rows();
  if (populated) {
    assert.deepEqual(values(), ['a@example.test', 'b@example.test', 'z@example.test']);
    if (editing) {
      assert.ok(document.body.textContent.includes('No unsaved changes'));
      await enter(email(initial[0]), 'changed@example.test');
      await enter(email(initial[0]), 'a@example.test');
      await until(() => document.body.textContent.includes('No unsaved changes'));
    }
    await enter(email(initial[0]), 'zz@example.test');
    email(initial[0]).blur();
    await nextTurn();
    assertRows(initial);
  } else {
    const name = document.querySelector('#name');
    await enter(name, 'Stable recipient order');
    add().click();
    await until(() => rows().length === 1);
    rows()[0].querySelector('button').click();
    await until(() => rows().length === 0);
  }
  add().click();
  await until(() => rows().length === initial.length + 1);
  const first = rows()[0];
  assert.equal(email(first).value, '');
  assertRows([first, ...initial]);
  for (const value of ['D', 'd@', 'd@example.test']) {
    await enter(email(first), value);
    assert.equal(rows()[0], first);
    assert.equal(document.activeElement, email(first));
    assertRows([first, ...initial]);
  }
  email(first).focus();
  for (const value of ['typing', 'without', 'waiting', 'd@example.test']) {
    email(first).value = value;
    email(first).dispatchEvent(new win.Event('input', { bubbles: true }));
  }
  await nextTurn();
  assert.equal(email(first).value, 'd@example.test', 'a burst of input retains the latest text');
  assert.equal(document.activeElement, email(first));
  assertRows([first, ...initial]);
  add().click();
  await until(() => rows().length === initial.length + 2);
  const second = rows()[0];
  assert.equal(rows()[1], first);
  const fields = [...first.querySelectorAll('input')];
  for (const [index, value] of [
    [1, 'Dana'],
    [2, 'Example'],
    [3, 'Analyst'],
  ]) {
    await enter(fields[index], value);
    fields[index].blur();
    await nextTurn();
    assertRows([second, first, ...initial]);
  }
  await enter(email(second), 'e@example.test');
  const save = () =>
    [...document.querySelectorAll('button')].find(
      (button) => button.textContent.trim() === (editing ? 'Save changes' : 'Create group'),
    );
  failSave = true;
  save().click();
  await until(
    () => writes.length === 1 && !save().disabled && document.activeElement === email(first),
  );
  assert.equal(email(first).getAttribute('aria-invalid'), 'true');
  assert.deepEqual(data.groups, originalGroups, 'a rejected save must not mutate stored groups');
  const errorId = email(first).getAttribute('aria-describedby');
  const removed = populated ? initial[0] : second;
  removed.querySelector('button').click();
  await until(() => rows().length === initial.length + 1);
  assert.ok(rows().includes(first), 'deletion preserves the surviving input element');
  assert.notEqual(
    email(first).getAttribute('aria-invalid'),
    'true',
    'structural edits invalidate old server diagnostics',
  );
  await enter(email(first), 'd@example.test');
  email(first).blur();
  await until(() => email(first).getAttribute('aria-invalid') !== 'true');
  const beforeSave = rows();
  const expected = populated
    ? ['e@example.test', 'd@example.test', 'b@example.test', 'z@example.test']
    : ['d@example.test'];
  assert.deepEqual(values(), expected);
  save().click();
  await until(() => writes.length === 2 && !save().disabled);
  assertRows(beforeSave);
  assert.deepEqual(values(), expected);
  await until(() => document.activeElement === email(first));
  assert.equal(
    email(first).getAttribute('aria-describedby'),
    errorId,
    'validation stays bound to the same input',
  );
  assert.equal(
    email(first).getAttribute('aria-invalid'),
    'true',
    'server diagnostics use the surviving recipient index',
  );
  assert.deepEqual(
    writes[1].targets.map((row) => row.email),
    expected,
    'save uses the displayed order',
  );
  assert.deepEqual(
    writes[1].targets.find((row) => row.email === 'd@example.test'),
    { email: 'd@example.test', first_name: 'Dana', last_name: 'Example', position: 'Analyst' },
  );
  assert.deepEqual(data.groups, originalGroups, 'a rejected retry must not mutate stored groups');
  failSave = false;
  save().click();
  await until(() => writes.length === 3 && /^\/groups\/\d+$/.test(win.location.pathname));
  assert.deepEqual(writes[2].targets, writes[1].targets);
  const savedId = Number(win.location.pathname.split('/').at(-1));
  const stored = data.groups.find((group) => group.id === savedId);
  assert.ok(stored, 'navigation opens the committed group identity');
  assert.deepEqual(stored.targets, writes[2].targets);
  assert.equal(stored.name, writes[2].name);
  if (editing) {
    assert.equal(savedId, 1);
    assert.equal(stored.revision, originalGroups.find((group) => group.id === 1).revision + 1);
  } else {
    assert.ok(!originalGroups.some((group) => group.id === savedId));
    assert.equal(stored.revision, 1);
    assert.deepEqual(
      data.groups.filter((group) => group.id !== savedId),
      originalGroups,
    );
  }
  const emailHeader = () =>
    [...document.querySelectorAll('button.table-sort')].find(
      (button) => button.textContent.trim() === 'Email',
    );
  await until(() => emailHeader());
  const savedTable = emailHeader().closest('table');
  const savedEmails = () =>
    [...savedTable.querySelectorAll('tbody tr')].map((row) =>
      row.firstElementChild.textContent.trim(),
    );
  assert.deepEqual(
    savedEmails(),
    [...expected].sort(),
    'read-only view keeps normal email sorting',
  );
  emailHeader().click();
  await nextTurn();
  assert.deepEqual(savedEmails(), [...expected].sort().reverse());
  process.exit(0);
}

if (settingsHidden) {
  const field = (label) =>
    [...document.querySelectorAll('label')]
      .find((element) => element.textContent.includes(label))
      ?.querySelector('input');
  const enabled = () => field('Require domain verification');
  const validity = () => field('Valid for (seconds)');
  const recheck = () => field('Recheck interval (seconds)');
  const target = scenario === 'settings-hidden-validity' ? validity : recheck;
  const other = scenario === 'settings-hidden-validity' ? recheck : validity;
  const corrected = scenario === 'settings-hidden-validity' ? '172800' : '10800';
  const save = () => document.querySelector('form button[type="submit"]');
  const enter = async (input, value) => {
    input.value = value;
    input.dispatchEvent(new win.Event('input', { bubbles: true }));
    await nextTurn();
  };
  await until(() => settled() && enabled() && save());
  assert.equal(enabled().checked, false);
  assert.equal(validity(), undefined);
  assert.equal(recheck(), undefined);
  enabled().click();
  await until(() => target());
  // An unfinished value can remain when its section is hidden before blur.
  await enter(target(), '0');
  assert.notEqual(target().getAttribute('aria-invalid'), 'true');
  enabled().click();
  await until(() => !target());
  assert.equal(save().disabled, false);
  const requestsBefore = requests.filter((path) => path === '/api/settings').length;
  save().click();
  await until(() => target() && document.activeElement === target());
  assert.equal(enabled().checked, false, 'an error reveals the section without changing policy');
  assert.equal(target().value, '0');
  assert.equal(target().getAttribute('aria-invalid'), 'true');
  const errorId = target().getAttribute('aria-describedby');
  assert.ok(errorId && document.getElementById(errorId)?.textContent.trim());
  assert.notEqual(other().getAttribute('aria-invalid'), 'true');
  assert.equal(
    document.querySelector('dialog[open]'),
    null,
    'invalid settings cannot reach confirmation',
  );
  assert.equal(requests.filter((path) => path === '/api/settings').length, requestsBefore);
  await enter(target(), corrected);
  await until(() => !target());
  assert.equal(document.getElementById(errorId), null);
  assert.equal(document.querySelector('input[aria-invalid="true"]'), null);
  assert.equal(document.querySelector('.validation-summary li'), null);
  enabled().click();
  await until(() => target());
  assert.equal(target().value, corrected, 'remounting preserves the corrected draft');
  assert.notEqual(target().getAttribute('aria-invalid'), 'true');
  enabled().click();
  await until(() => !target());
  save().click();
  await until(() => document.querySelector('dialog[open]'));
  assert.equal(document.querySelector('input[aria-invalid="true"]'), null);
  assert.equal(requests.filter((path) => path === '/api/settings').length, requestsBefore);
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
      ? 'Mailadresse,First Name,Last Name,Position\nz@example.test,Zoe,Example,Analyst\nalice@example.test,Alice,Example,Analyst\nb@example.test,Bob,Example,Analyst\n'
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
    const resourcePath = scenario === 'form-webhook' ? '/webhooks' : '/reporting-mailboxes';
    assert.ok(document.querySelector(`a[href="${resourcePath}"]`));
    assert.equal(document.querySelector('a[href="/settings"]'), null);
    featureOverrides[
      scenario === 'form-webhook' ? 'webhooks_enabled' : 'reporting_mailboxes_enabled'
    ] = false;
    await refreshSession();
    assert.equal(input.isConnected, false, 'actual feature disablement removes its form');
    assert.equal(document.querySelector(`a[href="${resourcePath}"]`), null);
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
    const recipientRows = () => [...document.querySelectorAll('.recipient-table tbody tr')];
    const loaded = recipientRows();
    assert.deepEqual(
      loaded.map((row) => row.querySelector('input').value),
      ['alice@example.test', 'b@example.test', 'z@example.test'],
    );
    assert.equal(document.querySelectorAll('.recipient-table thead button').length, 0);
    // The preference-refresh loop above deliberately leaves the interface in German.
    const addRecipient = [...document.querySelectorAll('button')].find(
      (button) => button.textContent.trim() === 'Empf\u00e4nger hinzuf\u00fcgen',
    );
    assert.ok(addRecipient, 'the imported editor exposes the translated add action');
    addRecipient.click();
    await until(() => recipientRows().length === 4);
    const added = recipientRows()[0];
    const address = added.querySelector('input');
    address.value = 'zz@example.test';
    address.dispatchEvent(new win.Event('input', { bubbles: true }));
    await nextTurn();
    assert.equal(recipientRows()[0], added);
    loaded.forEach((row, index) => assert.equal(recipientRows()[index + 1], row));
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

if (scenario.startsWith('recipient-') && !recipientEditor) {
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
if (scenario === 'sort-directory') {
  const preset = () =>
    [...document.querySelectorAll('button')].find(
      (b) => b.textContent.trim() === 'Recent work (drafts first)',
    );
  const firstLink = () =>
    document.querySelector('.management-table tbody th a')?.getAttribute('href');
  const rowLinks = () =>
    [...document.querySelectorAll('.management-table tbody th a')].map((a) =>
      a.getAttribute('href'),
    );
  const lastRequest = () =>
    new URL(
      requests.filter((r) => r.startsWith('/api/campaigns/directory?')).at(-1),
      'https://fixture.test',
    );
  await until(() => preset() && settled());
  assert.equal(preset().getAttribute('aria-pressed'), 'true');
  assert.equal(lastRequest().searchParams.get('sort'), 'recent');
  assert.equal(lastRequest().searchParams.get('order'), 'desc');
  assert.match(firstLink(), /^\/campaigns\/drafts\//);
  const recentFirstPage = rowLinks();
  [...document.querySelectorAll('button')]
    .find((b) => b.textContent.trim() === 'Next page')
    .click();
  await until(() => lastRequest().searchParams.get('after') && settled());
  document.querySelector('.management-table th .table-sort').click();
  await until(() => lastRequest().searchParams.get('sort') === 'name' && settled());
  assert.ok(!lastRequest().searchParams.get('after'), 'header sorting resets pagination');
  assert.equal(firstLink(), '/campaigns/1/results');
  const nameFirstPage = rowLinks();
  assert.equal(preset().getAttribute('aria-pressed'), 'false');
  [...document.querySelectorAll('button')]
    .find((b) => b.textContent.trim() === 'Next page')
    .click();
  await until(() => lastRequest().searchParams.get('after') && settled());
  preset().click();
  // Returning to a cached selection need not make another HTTP request.
  await until(
    () =>
      preset().getAttribute('aria-pressed') === 'true' &&
      firstLink() === recentFirstPage[0] &&
      settled(),
  );
  assert.deepEqual(rowLinks(), recentFirstPage, 'restoring workflow order returns its first page');
  assert.match(firstLink(), /^\/campaigns\/drafts\//);
  assert.equal(preset().getAttribute('aria-pressed'), 'true');
  // A manually selected order persists across route navigation.
  document.querySelector('.management-table th .table-sort').click();
  await until(() => firstLink() === '/campaigns/1/results' && settled());
  assert.deepEqual(rowLinks(), nameFirstPage, 'cached column sorting returns its first page');
  document.querySelector('.management-table tbody th a').click();
  await until(() => win.location.pathname === '/campaigns/1/results' && settled());
  document.querySelector('.admin-nav a[href="/campaigns"]').click();
  await until(() => win.location.pathname === '/campaigns' && preset() && settled());
  assert.equal(firstLink(), '/campaigns/1/results');
  assert.equal(preset().getAttribute('aria-pressed'), 'false');
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
