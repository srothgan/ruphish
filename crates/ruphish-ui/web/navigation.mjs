// Leptos owns routing. This adapter only protects drafts before its click/popstate
// listeners run; History API traversal cannot be canceled with preventDefault.
export function confirmDiscard(dirty, win = window) {
  return !dirty || win.confirm('Discard unsaved changes and leave this page?');
}

export function installNavigationGuard(isDirty, win = window) {
  const {history} = win;
  const marker = '__ruphishHistoryIndex';
  const originalPush = history.pushState;
  const originalReplace = history.replaceState;
  let index = Number.isInteger(history.state?.[marker]) ? history.state[marker] : 0;
  let restoring = false;
  const tagged = (state, value) => ({...state, [marker]: value});
  const confirm = () => confirmDiscard(isDirty(), win);
  originalReplace.call(history, tagged(history.state, index), '', win.location.href);

  function push(state, title, url) {
    originalPush.call(history, tagged(state, index + 1), title, url);
    index += 1;
  }
  function replace(state, title, url) {
    originalReplace.call(history, tagged(state, index), title, url);
  }
  history.pushState = push;
  history.replaceState = replace;

  const click = event => {
    if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const anchor = event.composedPath().find(node => node?.tagName === 'A');
    if (!anchor?.href || anchor.hasAttribute('download') || (anchor.target && anchor.target !== '_self')) return;
    const target = new URL(anchor.href, win.location.href);
    // Document navigation already has beforeunload; fragment-only changes retain the draft.
    if (target.origin !== win.location.origin || anchor.rel.split(/\s+/).includes('external')) return;
    if (target.pathname === win.location.pathname && target.search === win.location.search) return;
    if (restoring || !confirm()) {
      event.preventDefault();
      event.stopImmediatePropagation();
    }
  };
  const pop = event => {
    const target = event.state?.[marker];
    if (restoring) {
      event.stopImmediatePropagation();
      if (target === index) restoring = false;
      return;
    }
    // Entries outside this document use the browser's beforeunload lifecycle.
    if (!Number.isInteger(target) || target === index) return;
    if (!confirm()) {
      event.stopImmediatePropagation();
      restoring = true;
      history.go(index - target);
    } else index = target;
  };
  const unload = event => {
    if (isDirty()) { event.preventDefault(); event.returnValue = ''; }
  };
  win.addEventListener('click', click, true);
  win.addEventListener('popstate', pop, true);
  win.addEventListener('beforeunload', unload);
  return () => {
    win.removeEventListener('click', click, true);
    win.removeEventListener('popstate', pop, true);
    win.removeEventListener('beforeunload', unload);
    if (history.pushState === push) history.pushState = originalPush;
    if (history.replaceState === replace) history.replaceState = originalReplace;
  };
}
