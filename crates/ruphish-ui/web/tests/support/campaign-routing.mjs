import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { JSDOM } from 'jsdom';
import { createDemoApi, preferences } from '../../demo-api.mjs';
import { dataset } from '../../../../../assets/website-demo/dataset.mjs';

const scenario = process.argv[2];
const path = { 'completed-direct': '/campaigns/2/results', bookmark: '/campaigns/2?source=mailbox#history',
  dashboard: '/', directory: '/campaigns', invalid: '/campaigns/not-an-id', new: '/campaigns/new' }[scenario] ?? '/campaigns/2';
const dom = new JSDOM('<!doctype html><html><body></body></html>', {
  url: `https://admin.example.test${path}`, pretendToBeVisual: true,
});
const win = dom.window;
for (const key of Object.getOwnPropertyNames(win)) {
  if (!(key in globalThis)) {
    try { globalThis[key] = win[key]; } catch { /* Read-only host globals. */ }
  }
}
Object.defineProperty(globalThis, 'navigator', { value: win.navigator, configurable: true });
globalThis.addEventListener = win.addEventListener.bind(win);
globalThis.removeEventListener = win.removeEventListener.bind(win);
globalThis.ResizeObserver = class { observe() {} disconnect() {} };
win.ResizeObserver = globalThis.ResizeObserver;
win.matchMedia = () => ({ matches: false, addEventListener() {}, removeEventListener() {} });
win.scrollTo = () => {};
// Node Request needs the base URL that browsers supply for relative requests.
const NativeRequest = globalThis.Request;
globalThis.Request = class extends NativeRequest {
  constructor(input, init) { super(typeof input === 'string' ? new URL(input, win.location.href) : input, init); }
};
win.Request = globalThis.Request;
win.Response = globalThis.Response;
Object.assign(preferences, { language: 'de', format_locale: 'de-DE', time_zone: 'Europe/Berlin' });
const now = 1793016000;
const data = dataset(now);
const campaign = data.campaigns.find(c => c.header.id === 2);
campaign.header.status = scenario === 'active-entry' ? 'In progress' : scenario === 'queued-entry' ? 'Queued' : 'Completed';
const api = createDemoApi({ now, data, storage: { getItem: () => '1' } });
const requests = [];
let failure = scenario === 'missing' ? 404 : scenario === 'forbidden' ? 403 : scenario === 'retry' ? 503 : null;
let hold = false;
let release;
globalThis.fetch = async (request, options) => {
  const url = new URL(request.url);
  requests.push(url.pathname + url.search);
  if (failure && url.pathname === '/api/campaigns/2/context') {
    return new Response(JSON.stringify({ success: false, message: 'Unavailable', code: 'TEST_FAILURE' }), { status: failure });
  }
  if (hold && url.pathname === '/api/campaigns/2/results') await new Promise(resolve => { release = resolve; });
  return api.execute(request, options);
};
win.fetch = globalThis.fetch;
const dist = new URL('../../../../../dist/admin/', import.meta.url);
const html = readFileSync(new URL('index.html', dist), 'utf8');
const script = html.match(/import init, \* as bindings from '\/(.*?)'/)[1];
const { initSync } = await import(new URL(script, dist));
initSync({ module: readFileSync(new URL(script.replace(/\.js$/, '_bg.wasm'), dist)) });

async function until(condition) {
  const deadline = Date.now() + 5000;
  while (!condition()) {
    assert.ok(Date.now() < deadline, `Unmet condition at ${win.location.href}: ${win.document.body.textContent.slice(0,300)}`);
    await new Promise(resolve => setTimeout(resolve, 5));
  }
}
const document = win.document;
const regions = () => [...document.querySelectorAll('.query-region')];
const settled = () => regions().length > 0 && regions().every(e => e.dataset.ready === 'true' && e.getAttribute('aria-busy') === 'false');
const resultsPath = '/campaigns/2/results';

if (['missing', 'forbidden', 'retry', 'invalid'].includes(scenario)) {
  await until(() => document.querySelector('.status-page'));
  assert.equal(win.location.pathname, path);
  assert.equal(document.querySelector('.analysis-filters'), null);
  assert.ok(!requests.some(url => url.includes('/results')), 'do not fetch results before resolving the entry');
  if (scenario !== 'retry') process.exit(0);
  failure = null;
  document.querySelector('.status-page button').click();
}
if (scenario === 'new') {
  await until(() => document.querySelector('.step-navigation'));
  assert.equal(win.location.pathname, '/campaigns/new');
  assert.ok(!requests.some(url => url.startsWith('/api/campaigns/new')));
  process.exit(0);
}
if (['dashboard', 'directory'].includes(scenario)) {
  await until(() => document.querySelector('a[href="/campaigns/2/results"]'));
  const listed = scenario === 'dashboard'
    ? data.campaigns.filter(c => [...document.querySelectorAll('.dashboard-campaigns a')].some(link => link.textContent === c.header.name))
    : data.campaigns;
  assert.ok(listed.some(c => c.header.status === 'Completed'));
  assert.ok(listed.some(c => c.header.status !== 'Completed'));
  for (const item of listed) {
    const suffix = item.header.status === 'Completed' ? 'results' : 'delivery';
    const links = [...document.querySelectorAll(`a[href^="/campaigns/${item.header.id}"]`)];
    assert.ok(links.some(link => link.getAttribute('href') === `/campaigns/${item.header.id}/${suffix}`));
    assert.ok(links.every(link => link.getAttribute('href') !== `/campaigns/${item.header.id}`), 'no duplicate results entry links');
  }
  document.querySelector('a[href="/campaigns/2/results"]').click();
}
if (['active-entry', 'queued-entry'].includes(scenario)) {
  await until(() => win.location.pathname === '/campaigns/2/delivery' && settled());
  assert.equal(win.history.length, 1, 'entry redirect replaces history');
  assert.ok(!requests.some(url => url.includes('/results')), 'delivery entry does not mount results');
  document.querySelector('.campaign-sections a[href="/campaigns/2/results"]').click();
}
await until(() => win.location.pathname === resultsPath && settled() && document.querySelector('.analysis-filters'));
if (scenario === 'bookmark') {
  assert.equal(win.location.search, '?source=mailbox');
  assert.equal(win.location.hash, '#history');
}
if (['completed-entry', 'bookmark', 'retry'].includes(scenario)) assert.equal(win.history.length, 1);
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
  assert.equal(document.querySelector('.analysis-filters'), form, 'filtering must not remount the form');
  assert.equal(document.querySelector('h1'), heading, 'filtering must not remount the campaign header');
  assert.ok(!controls.hidden);
  assert.ok(regions().every(e => e.dataset.ready === 'true'), 'no initial loading during filter transitions');
  assert.equal(document.querySelector('.query-placeholder'), null);
  assert.equal(form.querySelector('button[type="submit"]').getAttribute('aria-busy'), 'true');
  hold = false;
  release(); release = null;
  await until(settled);
}
process.exit(0);
