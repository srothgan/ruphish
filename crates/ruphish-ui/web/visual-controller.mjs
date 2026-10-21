import {editorText, editorError, localizeVisualShell, contentLocale} from './editor-i18n.mjs';
import {VisualSession, visualOptions} from './visual-session.mjs';

// Library creation is injected so the actual controller can also exercise startup failures.
export function installVisualController(window, createEditor, send = message => window.parent.postMessage(message, '*')) {
  const document = window.document;
  const status = document.querySelector('#status');
  const editing = document.querySelector('#editing');
  let session, editor, token, variables = [];
  let locale = document.documentElement.lang || 'en', statusKey = 'frame_loading', lastError = '';
  const t = key => editorText(locale, key);
  const setStatus = key => { statusKey = key; status.hidden = !key; status.textContent = (key ? t(key) : '') + (lastError ? ` ${editorError(locale, lastError)}` : ''); };
  const message = (type, extra = {}) => send({type, token, ...extra});
  const showError = error => {
    status.dataset.error = '';
    status.setAttribute('role', 'alert');
    lastError = error; setStatus('failed'); message('invalid');
  };
  const publish = (type = 'change') => {
    if (!session) return;
    try {
      const source = session.result();
      lastError = ''; delete status.dataset.error; status.setAttribute('role', 'status'); setStatus(null);
      message(type, {source});
    } catch (error) { showError(error); }
  };
  const run = action => {
    if (!session) return;
    try { action(); publish(); }
    catch (error) { showError(error); }
  };
  const onMessage = event => {
    if (event.source !== window.parent) return;
    const data = event.data;
    if (data?.type === 'locale' && data.token === token && typeof data.locale === 'string') {
      // TODO(Jodit 4.18.4): refresh vendor toolbar/dialog language in place once
      // upstream supports it without resetting selection, undo history or the draft.
      // Existing vendor widgets retain their opening language; reopening uses the new one.
      locale = data.locale;
      localizeVisualShell(document, locale);
      setStatus(statusKey);
      return;
    }
    if (data?.type === 'snapshot' && data.token === token) { publish('snapshot'); return; }
    if (data?.type === 'insert' && data.token === token && variables.includes(data.name)) {
      run(() => session.insertVariable(data.name)); return;
    }
    if (data?.type !== 'open' || typeof data.source !== 'string' || typeof data.token !== 'string' || !Array.isArray(data.variables) || !data.variables.every(name => typeof name === 'string')) return;
    token = data.token; variables = data.variables;
    locale = typeof data.locale === 'string' ? data.locale : 'en'; lastError = '';
    localizeVisualShell(document, locale);
    session = null; editing.disabled = true; editing.hidden = true;
    delete status.dataset.error; status.setAttribute('role', 'status');
    setStatus('frame_loading');
    try {
      editor?.destruct(); editor = null;
      editor = createEditor(document.querySelector('#visual'), visualOptions(window, data.source, locale));
      session = new VisualSession(editor, data.source);
      editor.e.on('change afterCommand', () => publish());
      editor.editor.addEventListener('input', () => publish());
      editing.hidden = false; editing.disabled = false;
      setStatus(null);
      message('opened');
    } catch (error) {
      editor?.destruct(); editor = null; session = null;
      showError(error); message('failed');
    }
  };
  window.addEventListener('message', onMessage);
  const form = document.querySelector('#form');
  if (form) {
  document.querySelector('#load-field').onclick = () => run(() => {
    const form = document.querySelector('#form');
    for (const [name, value] of Object.entries(session.selectedProperties())) form.elements.namedItem(name).value = value;
    setStatus('field_loaded');
  });
  document.querySelector('#update-field').onclick = () => run(() => {
    session.updateSelected(Object.fromEntries(new window.FormData(document.querySelector('#form'))));
    setStatus('field_updated');
  });
  document.querySelector('#form').onsubmit = event => {
    event.preventDefault();
    run(() => {
      session.insertForm({...Object.fromEntries(new window.FormData(event.currentTarget)),
        submitLabel: editorText(contentLocale(editor.ed, locale), "continue_label")});
      setStatus('form_inserted');
    });
  };
  }
  message('booted');
  return () => {
    window.removeEventListener('message', onMessage);
    editor?.destruct(); editor = null; session = null;
  };
}
