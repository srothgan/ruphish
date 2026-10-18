import {test} from 'node:test';
import assert from 'node:assert/strict';
import {JSDOM} from 'jsdom';
import {installNavigationGuard} from '../navigation.mjs';

function fixture() {
  const dom = new JSDOM('<a href="/templates"><svg><path/></svg>Templates</a>', {url: 'https://admin.example.test/groups'});
  const win = dom.window;
  const visits = [];
  // Model the router's existing bubbling listeners, registered before the guard.
  win.addEventListener('click', event => {
    if (!event.defaultPrevented && !event.ctrlKey && event.button === 0) visits.push('click');
    event.preventDefault(); // JSDOM must not attempt a real document navigation.
  });
  win.addEventListener('popstate', () => visits.push(win.location.pathname));
  let dirty = false;
  let answer = false;
  let prompts = 0;
  win.confirm = () => { prompts++; return answer; };
  const remove = installNavigationGuard(() => dirty, win);
  return {win, visits, remove, setDirty: value => { dirty = value; }, accept: () => { answer = true; }, prompts: () => prompts, close: () => { remove(); win.close(); }};
}

// Resolve on an observed history event rather than sleeping for a guessed delay.
function traverse(win, delta, expected) {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => { win.removeEventListener('popstate', listener, true); reject(new Error('History did not settle')); }, 2000);
    const listener = () => {
      if (win.location.pathname === expected) {
        clearTimeout(timeout);
        win.removeEventListener('popstate', listener, true);
        resolve();
      }
    };
    // Register before a guard when observing suppressed restoration events.
    win.addEventListener('popstate', listener, true);
    win.history.go(delta);
  });
}

test('clean links reach the router; canceled dirty navigation never reaches it, including SVG clicks', () => {
  const f = fixture();
  try {
    const click = options => f.win.document.querySelector('path').dispatchEvent(new f.win.MouseEvent('click', {bubbles: true, cancelable: true, ...options}));
    click();
    assert.deepEqual(f.visits, ['click']);
    assert.equal(f.prompts(), 0);
    f.setDirty(true);
    click();
    assert.deepEqual(f.visits, ['click']);
    assert.equal(f.prompts(), 1);
    click({ctrlKey: true});
    assert.equal(f.prompts(), 1, 'Opening another tab must preserve the current draft without prompting');
    f.accept();
    click();
    assert.deepEqual(f.visits, ['click', 'click']);
  } finally { f.close(); }
});

test('downloads and same-page fragments do not discard drafts; unload does and cleanup removes the guard', () => {
  const f = fixture();
  try {
    f.setDirty(true);
    const link = f.win.document.querySelector('a');
    link.href = '#content';
    link.click();
    link.href = '/export';
    link.download = 'resource.json';
    link.click();
    assert.equal(f.prompts(), 0);
    const blocked = new f.win.Event('beforeunload', {cancelable: true});
    f.win.dispatchEvent(blocked);
    assert.equal(blocked.defaultPrevented, true);
    f.setDirty(false); // Represents save/cancel or terminal session invalidation.
    const clean = new f.win.Event('beforeunload', {cancelable: true});
    f.win.dispatchEvent(clean);
    assert.equal(clean.defaultPrevented, false);
    f.setDirty(true);
    f.remove();
    const removed = new f.win.Event('beforeunload', {cancelable: true});
    f.win.dispatchEvent(removed);
    assert.equal(removed.defaultPrevented, false);
  } finally { f.close(); }
});

test('clean Back and Forward reach the router and replacement preserves the history entry', async () => {
  const f = fixture();
  try {
    f.win.history.pushState({sample: 1}, '', '/templates/new');
    f.win.history.replaceState({sample: 2}, '', '/templates/42');
    assert.equal(f.win.history.state.sample, 2);
    await traverse(f.win, -1, '/groups');
    await traverse(f.win, 1, '/templates/42');
    assert.deepEqual(f.visits, ['/groups', '/templates/42']);
    assert.equal(f.win.history.length, 2);
  } finally { f.close(); }
});

test('canceling a multi-entry Back restores the URL without unmounting the draft; acceptance later traverses normally', async () => {
  const dom = new JSDOM('', {url: 'https://admin.example.test/groups'});
  const win = dom.window;
  let observed;
  // Capture observation installed first so it can also see suppressed restoration.
  win.addEventListener('popstate', () => observed?.(), true);
  let accepted = false;
  win.confirm = () => accepted;
  const visits = [];
  win.addEventListener('popstate', () => visits.push(win.location.pathname));
  const remove = installNavigationGuard(() => true, win);
  const settled = path => new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error('History restoration did not settle')), 2000);
    observed = () => { if (win.location.pathname === path) { clearTimeout(timeout); resolve(); } };
  });
  try {
    win.history.pushState(null, '', '/templates');
    win.history.pushState(null, '', '/templates/42');
    const restored = settled('/templates/42');
    win.history.go(-2);
    await restored;
    assert.deepEqual(visits, []);
    assert.equal(win.history.length, 3);
    accepted = true;
    await traverse(win, -2, '/groups');
    assert.deepEqual(visits, ['/groups']);
  } finally { remove(); win.close(); }
});
