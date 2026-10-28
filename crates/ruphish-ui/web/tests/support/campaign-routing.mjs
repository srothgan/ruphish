import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { JSDOM } from 'jsdom';
import { createDemoApi, preferences } from '../../demo-api.mjs';
import { dataset } from '../../../../../assets/website-demo/dataset.mjs';

const scenario = process.argv[2];
const path = { 'event-editor': '/campaigns/2/delivery?recipient=20003', 'completed-direct': '/campaigns/2/results', bookmark: '/campaigns/2?source=mailbox#history',
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
win.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
win.HTMLDialogElement.prototype.close = function () { this.open = false; };
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
const changes = [];
let failure = scenario === 'missing' ? 404 : scenario === 'forbidden' ? 403 : scenario === 'retry' ? 503 : null;
let hold = false;
let release;
globalThis.fetch = async (request, options) => {
  const url = new URL(request.url);
  requests.push(url.pathname + url.search);
  if (request.method === 'POST' && url.pathname.endsWith('/events')) changes.push(await request.clone().json());
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

if (scenario === 'event-editor') {
  await until(() => settled() && document.querySelector('.recipient-history .delivery-timeline li'));
  document.querySelector('.recipient-history > .recipient-history-heading button').click();
  await until(() => document.querySelector('.event-review'));
  const review = document.querySelector('.event-review');
  const recipient = campaign.results.find(r => r.id === 20003);
  const initialCount = recipient.events.length;
  const change = (control, value, type = 'change') => {
    control.value = value;
    control.dispatchEvent(new win.Event(type, { bubbles: true }));
  };
  const openAdd = async () => {
    review.querySelector(':scope > button').click();
    await until(() => document.querySelector('form.event-correction'));
    const form = document.querySelector('form.event-correction');
    assert.equal(form.closest('dialog').querySelector('table').contains(form), false, 'the editor is outside the dialog event table');
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
  const localMinute = time => new Intl.DateTimeFormat('sv-SE', {
    timeZone: 'Europe/Berlin', year: 'numeric', month: '2-digit', day: '2-digit',
    hour: '2-digit', minute: '2-digit', hourCycle: 'h23',
  }).format(new Date(time)).replace(' ', 'T');
  const beforeAdd = localMinute(Date.now());
  form = await openAdd();
  const occurrence = form.querySelector('input');
  assert.ok(occurrence.value.slice(0, 16) >= beforeAdd
    && occurrence.value.slice(0, 16) <= localMinute(Date.now()), 'new events default to the current time, even in a completed campaign');
  // Pick a historical occurrence before submitting to the completed fixture campaign.
  const originalTime = localMinute(recipient.events.at(-1).time);
  change(occurrence, '', 'input');
  assert.equal(form.checkValidity(), false, 'an empty occurrence time cannot submit');
  change(occurrence, originalTime, 'input');
  change(form.querySelector('select'), 'form_submitted');
  await until(() => form.querySelector('.event-correction-presence'));
  change(form.querySelectorAll('select')[1], 'true');
  change(form.querySelectorAll('select')[2], 'false');
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
  const operatorRow = () => [...review.querySelectorAll('tr:has(td)')].find(row => row.cells[2]?.textContent.trim() === 'Manuell erfasst');
  assert.ok(operatorRow(), 'operator creation is visible');
  assert.deepEqual([...operatorRow().querySelectorAll('.field-presence small')].map(e => e.textContent), ['Keine Eingabe erkannt', 'Eingegeben']);
  const submittedTimeline = [...document.querySelectorAll('.timeline-event')].find(row => row.querySelector('.submission-fields'));
  assert.ok(submittedTimeline, 'delivery history shows submission field indicators');

  assert.equal(operatorRow().cells[3].textContent.trim(), '', 'creating an operator event must not show Edited');
  assert.deepEqual(changes[0].value.presence, { username_present: false, password_present: true });
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
  operatorRow().querySelectorAll('.table-actions button')[1].click();
  await until(() => document.querySelector('form.event-correction'));
  form = document.querySelector('form.event-correction');
  form.dispatchEvent(new win.Event('submit', { bubbles: true, cancelable: true }));
  await until(() => !document.querySelector('form.event-correction'));
  assert.equal(changes.at(-1).action, 'delete');
  assert.equal(recipient.events.length, initialCount);
  assert.ok(recipient.events.every(e => e.id !== added.id));
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
  assert.equal(form.querySelector('select').value, 'link_clicked', 'Keep editing retains the draft');
  dialog.dispatchEvent(new win.Event('cancel', { cancelable: true }));
  await until(() => confirmation.open);
  assert.equal(dialog.open, true, 'Escape must wait for confirmation before closing');
  confirmation.querySelector('[data-discard="confirm"]').click();
  await until(() => !document.querySelector('.event-edit-dialog'));
  assert.equal(changes.length, savedChanges, 'discarding must not save');
  assert.equal(recipient.events.length, initialCount);
  document.querySelector('.recipient-history > .recipient-history-heading button').click();
  await until(() => document.querySelector('.event-review'));
  assert.equal(document.querySelector('form.event-correction'), null, 'reopening must not restore the discarded draft');
  document.querySelector('.event-edit-dialog .recipient-history-heading button').click();
  await until(() => !document.querySelector('.event-edit-dialog'));
  assert.equal(confirmation.open, false, 'closing without a draft needs no confirmation');
  process.exit(0);
}

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
