import { test } from 'node:test';
import assert from 'node:assert/strict';
import { JSDOM } from 'jsdom';
import { confirmDiscard, installNavigationGuard } from '../navigation.mjs';

function fixture() {
  const dom = new JSDOM(
    '<a href="/templates"><svg><path/></svg>Templates</a><input value="Unsaved draft"><dialog><button data-discard="cancel" autofocus>Keep editing</button><button data-discard="confirm">Discard and continue</button></dialog>',
    { url: 'https://admin.example.test/groups' },
  );
  const win = dom.window;
  const dialog = win.document.querySelector('dialog');
  // JSDOM lacks modal dialog methods. Model only their lifecycle; the production
  // adapter and history/router events below execute unchanged.
  let prompts = 0;
  dialog.showModal = () => {
    dialog.open = true;
    prompts++;
    dialog.querySelector('[autofocus]').focus();
    dialog.dispatchEvent(new win.Event('opened'));
  };
  dialog.close = () => {
    dialog.open = false;
    dialog.dispatchEvent(new win.Event('close'));
  };
  win.confirm = () => {
    throw new Error('Native confirmation must not be used');
  };
  const visits = [];
  const historyEvents = new win.EventTarget();
  win.addEventListener(
    'popstate',
    () => historyEvents.dispatchEvent(new win.Event('observed')),
    true,
  );
  win.addEventListener('click', (event) => {
    if (
      !event.defaultPrevented &&
      !event.ctrlKey &&
      event.button === 0 &&
      event.target.closest?.('a')
    ) {
      visits.push('click');
    }
    event.preventDefault();
  });
  win.addEventListener('popstate', () => visits.push(win.location.pathname));
  let dirty = false;
  const remove = installNavigationGuard(() => dirty, dialog, win);
  return {
    win,
    dialog,
    visits,
    historyEvents,
    remove,
    setDirty: (value) => {
      dirty = value;
    },
    prompts: () => prompts,
    choose: (accepted) =>
      dialog.querySelector(`[data-discard="${accepted ? 'confirm' : 'cancel'}"]`).click(),
    close: () => {
      remove();
      win.close();
    },
  };
}

const flush = async () => {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
};
function nextEvent(target, type) {
  return new Promise((resolve) => {
    target.addEventListener(type, resolve, { once: true });
  });
}
async function traverse(win, delta, expected) {
  const event = nextEvent(win, 'popstate');
  win.history.go(delta);
  await event;
  assert.equal(win.location.pathname, expected);
}

test('dirty SVG link clicks wait for our modal; cancel preserves draft and focus, acceptance resumes once', async () => {
  const f = fixture();
  try {
    const click = (options) =>
      f.win.document
        .querySelector('path')
        .dispatchEvent(
          new f.win.MouseEvent('click', { bubbles: true, cancelable: true, ...options }),
        );
    click();
    assert.deepEqual(f.visits, ['click']);
    f.setDirty(true);
    const input = f.win.document.querySelector('input');
    input.focus();
    click();
    assert.equal(f.dialog.open, true);
    assert.equal(f.win.document.activeElement.textContent, 'Keep editing');
    assert.deepEqual(f.visits, ['click']);
    click();
    assert.equal(f.prompts(), 1, 'Repeated clicks must not queue extra navigation');
    f.choose(false);
    await flush();
    assert.equal(input.value, 'Unsaved draft');
    assert.equal(f.win.document.activeElement, input);
    assert.deepEqual(f.visits, ['click']);
    click({ ctrlKey: true });
    assert.equal(f.prompts(), 1);
    click();
    f.choose(true);
    await flush();
    assert.deepEqual(f.visits, ['click', 'click']);
    assert.equal(f.dialog.open, false);
  } finally {
    f.close();
  }
});

test('Escape and direct sign-out use the same modal; duplicate actions cannot reuse one answer', async () => {
  const f = fixture();
  try {
    assert.equal(await confirmDiscard(false, f.win), true);
    const first = confirmDiscard(true, f.win);
    assert.equal(await confirmDiscard(true, f.win), false);
    const escape = new f.win.Event('cancel', { cancelable: true });
    f.dialog.dispatchEvent(escape);
    assert.equal(escape.defaultPrevented, true);
    assert.equal(await first, false);
    const second = confirmDiscard(true, f.win);
    f.choose(true);
    assert.equal(await second, true);
    const interrupted = confirmDiscard(true, f.win);
    f.remove();
    assert.equal(await interrupted, false);
    assert.equal(await confirmDiscard(true, f.win), false, 'Missing guard must fail closed');
  } finally {
    f.close();
  }
});

test('downloads, new tabs and fragments keep drafts; document unload retains native protection', () => {
  const f = fixture();
  try {
    f.setDirty(true);
    const link = f.win.document.querySelector('a');
    link.href = '#content';
    link.click();
    link.href = '/export';
    link.download = 'resource.json';
    link.click();
    link.removeAttribute('download');
    link.target = '_blank';
    link.click();
    assert.equal(f.prompts(), 0);
    const blocked = new f.win.Event('beforeunload', { cancelable: true });
    f.win.dispatchEvent(blocked);
    assert.equal(blocked.defaultPrevented, true);
    f.setDirty(false);
    const clean = new f.win.Event('beforeunload', { cancelable: true });
    f.win.dispatchEvent(clean);
    assert.equal(clean.defaultPrevented, false);
    f.setDirty(true);
    f.remove();
    const removed = new f.win.Event('beforeunload', { cancelable: true });
    f.win.dispatchEvent(removed);
    assert.equal(removed.defaultPrevented, false);
  } finally {
    f.close();
  }
});

test('clean Back and Forward reach the router and replacement preserves the history entry', async () => {
  const f = fixture();
  try {
    f.win.history.pushState({ sample: 1 }, '', '/templates/new');
    f.win.history.replaceState({ sample: 2 }, '', '/templates/42');
    assert.equal(f.win.history.state.sample, 2);
    await traverse(f.win, -1, '/groups');
    await traverse(f.win, 1, '/templates/42');
    assert.deepEqual(f.visits, ['/groups', '/templates/42']);
    assert.equal(f.win.history.length, 2);
  } finally {
    f.close();
  }
});

test('multi-entry Back restores the URL before prompting; cancel keeps the draft and accept traverses once', async () => {
  const f = fixture();
  try {
    f.win.history.pushState(null, '', '/templates');
    f.win.history.pushState(null, '', '/templates/42');
    f.setDirty(true);
    let opened = nextEvent(f.dialog, 'opened');
    f.win.history.go(-2);
    await opened;
    assert.equal(f.win.location.pathname, '/templates/42');
    assert.deepEqual(f.visits, []);
    f.choose(false);
    await flush();
    assert.equal(f.win.history.length, 3);
    opened = nextEvent(f.dialog, 'opened');
    f.win.history.go(-2);
    await opened;
    const navigated = nextEvent(f.win, 'popstate');
    f.choose(true);
    await navigated;
    assert.deepEqual(f.visits, ['/groups']);
    assert.equal(f.win.history.length, 3);
    opened = nextEvent(f.dialog, 'opened');
    f.win.history.go(2);
    await opened;
    assert.equal(f.win.location.pathname, '/groups');
    const forward = nextEvent(f.win, 'popstate');
    f.choose(true);
    await forward;
    assert.deepEqual(f.visits, ['/groups', '/templates/42']);
  } finally {
    f.close();
  }
});

test('stale confirmation cannot navigate after cleanup, session invalidation or a programmatic route change', async () => {
  for (const interrupt of [
    (f) => f.remove(),
    (f) => f.setDirty(false),
    (f) => f.win.history.pushState(null, '', '/login'),
  ]) {
    const f = fixture();
    try {
      f.setDirty(true);
      f.win.document.querySelector('a').click();
      interrupt(f);
      f.choose(true);
      await flush();
      assert.deepEqual(f.visits, []);
    } finally {
      f.close();
    }
  }
});

test('another Back while the modal is open restores the original draft without queuing another destination', async () => {
  const f = fixture();
  try {
    f.win.history.pushState(null, '', '/templates');
    f.win.history.pushState(null, '', '/templates/42');
    f.setDirty(true);
    const opened = nextEvent(f.dialog, 'opened');
    f.win.history.back();
    await opened;
    const attempted = nextEvent(f.historyEvents, 'observed');
    f.win.history.go(-2);
    await attempted;
    await nextEvent(f.historyEvents, 'observed');
    assert.equal(f.win.location.pathname, '/templates/42');
    assert.equal(f.prompts(), 1);
    const navigated = nextEvent(f.win, 'popstate');
    f.choose(true);
    await navigated;
    assert.deepEqual(f.visits, ['/templates']);
  } finally {
    f.close();
  }
});

test('delayed close events cannot dismiss a new prompt; changed link destinations are not authorized', async () => {
  const f = fixture();
  try {
    const first = confirmDiscard(true, f.win);
    f.choose(false);
    await first;
    const second = confirmDiscard(true, f.win);
    f.dialog.dispatchEvent(new f.win.Event('close'));
    assert.equal(f.dialog.open, true);
    f.choose(true);
    assert.equal(await second, true);
    f.setDirty(true);
    const link = f.win.document.querySelector('a');
    link.click();
    link.href = '/users';
    f.choose(true);
    await flush();
    assert.deepEqual(f.visits, []);
  } finally {
    f.close();
  }
});
