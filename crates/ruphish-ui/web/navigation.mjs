// Leptos owns dialog markup and routing. Only document unloads use a browser warning.
const prompts = new WeakMap();

export async function confirmDiscard(dirty, win = window) {
  return !dirty || (await prompts.get(win)?.()) === true;
}

export function installNavigationGuard(isDirty, dialog, win = window) {
  const { history } = win;
  const marker = '__ruphishHistoryIndex';
  const originalPush = history.pushState;
  const originalReplace = history.replaceState;
  let index = Number.isInteger(history.state?.[marker]) ? history.state[marker] : 0;
  let disposed = false;
  let restoring = false;
  let restored = [];
  let pending = false;
  let approvedTarget = null;
  let replay = null;
  let resolvePrompt = null;
  let previousFocus = null;
  const tagged = (state, value) => ({ ...state, [marker]: value });
  const finish = (accepted) => {
    const resolve = resolvePrompt;
    if (!resolve) {
      return;
    }
    resolvePrompt = null;
    dialog.close();
    if (previousFocus?.isConnected) {
      previousFocus.focus();
    }
    resolve(accepted);
  };
  const prompt = () => {
    // One answer must never authorize multiple queued navigation/sign-out actions.
    if (disposed || resolvePrompt) {
      return Promise.resolve(false);
    }
    previousFocus = win.document.activeElement;
    return new Promise((resolve) => {
      resolvePrompt = resolve;
      try {
        dialog.showModal();
      } catch {
        finish(false);
      }
    });
  };
  const cancel = (event) => {
    event.preventDefault();
    finish(false);
  };
  const choose = (event) => {
    const button = event.target.closest('[data-discard]');
    if (button) {
      finish(button.dataset.discard === 'confirm');
    }
  };
  // A close event can arrive after another prompt has already opened.
  const closed = () => {
    if (!dialog.open) {
      finish(false);
    }
  };
  dialog.addEventListener('click', choose);
  dialog.addEventListener('cancel', cancel);
  dialog.addEventListener('close', closed);
  prompts.set(win, prompt);
  originalReplace.call(history, tagged(history.state, index), '', win.location.href);

  function push(state, title, url) {
    originalPush.call(history, tagged(state, index + 1), title, url);
    index += 1;
    approvedTarget = null;
    finish(false);
  }
  function replace(state, title, url) {
    originalReplace.call(history, tagged(state, index), title, url);
    approvedTarget = null;
    finish(false);
  }
  history.pushState = push;
  history.replaceState = replace;

  const block = (event) => {
    event.preventDefault();
    event.stopImmediatePropagation();
  };
  const click = (event) => {
    if (
      event.defaultPrevented ||
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    ) {
      return;
    }
    const anchor = event.composedPath().find((node) => node?.tagName === 'A');
    if (
      !anchor?.href ||
      anchor.hasAttribute('download') ||
      (anchor.target && anchor.target !== '_self')
    ) {
      return;
    }
    if (replay === anchor) {
      replay = null;
      return;
    }
    const target = new URL(anchor.href, win.location.href);
    if (target.origin !== win.location.origin || anchor.rel.split(/\s+/).includes('external')) {
      return;
    }
    if (target.pathname === win.location.pathname && target.search === win.location.search) {
      return;
    }
    if (!isDirty() && !pending && !restoring) {
      return;
    }
    block(event);
    if (pending || restoring) {
      return;
    }
    pending = true;
    const source = win.location.href;
    void confirmDiscard(isDirty(), win).then((accepted) => {
      pending = false;
      if (
        accepted &&
        !disposed &&
        isDirty() &&
        source === win.location.href &&
        anchor.isConnected &&
        anchor.href === target.href
      ) {
        replay = anchor;
        anchor.click();
        replay = null;
      }
    });
  };
  const settled = () =>
    restoring ? new Promise((resolve) => restored.push(resolve)) : Promise.resolve();
  const pop = (event) => {
    const target = event.state?.[marker];
    if (restoring && target === index) {
      event.stopImmediatePropagation();
      restoring = false;
      const listeners = restored;
      restored = [];
      for (const resolve of listeners) {
        resolve();
      }
      return;
    }
    // Entries outside this document use the browser's beforeunload lifecycle.
    if (!Number.isInteger(target) || target === index) {
      return;
    }
    if (approvedTarget === target) {
      approvedTarget = null;
      index = target;
      return;
    }
    if (!isDirty() && !pending) {
      index = target;
      return;
    }
    event.stopImmediatePropagation();
    restoring = true;
    history.go(index - target);
    if (pending) {
      return;
    }
    pending = true;
    void (async () => {
      // Restore the original URL first, keeping the router and draft untouched.
      await settled();
      if (disposed) {
        return;
      }
      const accepted = await confirmDiscard(isDirty(), win);
      await settled();
      pending = false;
      if (accepted && !disposed && isDirty()) {
        approvedTarget = target;
        history.go(target - index);
      }
    })();
  };
  const unload = (event) => {
    if (isDirty()) {
      event.preventDefault();
      event.returnValue = '';
    }
  };
  win.addEventListener('click', click, true);
  win.addEventListener('popstate', pop, true);
  win.addEventListener('beforeunload', unload);
  return () => {
    disposed = true;
    finish(false);
    for (const resolve of restored) {
      resolve();
    }
    restored = [];
    if (prompts.get(win) === prompt) {
      prompts.delete(win);
    }
    dialog.removeEventListener('click', choose);
    dialog.removeEventListener('cancel', cancel);
    dialog.removeEventListener('close', closed);
    win.removeEventListener('click', click, true);
    win.removeEventListener('popstate', pop, true);
    win.removeEventListener('beforeunload', unload);
    if (history.pushState === push) {
      history.pushState = originalPush;
    }
    if (history.replaceState === replace) {
      history.replaceState = originalReplace;
    }
  };
}
