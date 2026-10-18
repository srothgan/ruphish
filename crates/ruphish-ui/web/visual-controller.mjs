import {VisualSession, visualOptions} from './visual-session.mjs';

// Library creation is injected so the actual controller can also exercise startup failures.
export function installVisualController(window, createEditor, send = message => window.parent.postMessage(message, '*')) {
  const document = window.document;
  const status = document.querySelector('#status');
  const editing = document.querySelector('#editing');
  const apply = document.querySelector('#apply');
  const picker = document.querySelector('#variable');
  let session, editor, token, variables = [];
  const message = (type, extra = {}) => send({type, token, ...extra});
  const showError = error => {
    status.dataset.error = '';
    status.setAttribute('role', 'alert');
    status.textContent = `Visual editing failed. Your HTML source is unchanged. Choose “Cancel visual edits” above to return. ${error}`;
  };
  const run = action => {
    if (!session) return;
    try { action(); }
    catch (error) { showError(error); }
  };
  const onMessage = event => {
    if (event.source !== window.parent) return;
    const data = event.data;
    if (data?.type === 'insert' && data.token === token && variables.includes(data.name)) {
      run(() => session.insertVariable(data.name)); return;
    }
    if (data?.type !== 'open' || typeof data.source !== 'string' || typeof data.token !== 'string' || !Array.isArray(data.variables) || !data.variables.every(name => typeof name === 'string')) return;
    token = data.token; variables = data.variables;
    session = null; editing.disabled = true; editing.hidden = true; apply.disabled = true; picker.disabled = true;
    delete status.dataset.error; status.setAttribute('role', 'status');
    status.textContent = 'Loading the visual editor…';
    picker.replaceChildren(new window.Option('Choose a field…', ''));
    try {
      editor?.destruct(); editor = null;
      editor = createEditor(document.querySelector('#visual'), visualOptions(window, data.source));
      session = new VisualSession(editor, data.source);
      for (const name of variables) picker.append(new window.Option(name, name));
      editing.hidden = false; editing.disabled = false; apply.disabled = false; picker.disabled = false;
      status.textContent = 'Visual editing. Changes stay here until you apply them.';
      message('opened');
    } catch (error) {
      editor?.destruct(); editor = null; session = null;
      showError(error); message('failed');
    }
  };
  window.addEventListener('message', onMessage);
  picker.onchange = () => run(() => {
    if (variables.includes(picker.value)) session.insertVariable(picker.value);
    picker.value = '';
  });
  apply.onclick = () => run(() => message('apply', {source: session.result()}));
  document.querySelector('#load-field').onclick = () => run(() => {
    const form = document.querySelector('#form');
    for (const [name, value] of Object.entries(session.selectedProperties())) form.elements.namedItem(name).value = value;
    status.textContent = 'Selected field loaded.';
  });
  document.querySelector('#update-field').onclick = () => run(() => {
    session.updateSelected(Object.fromEntries(new window.FormData(document.querySelector('#form'))));
    status.textContent = 'Selected field updated.';
  });
  document.querySelector('#form').onsubmit = event => {
    event.preventDefault();
    run(() => {
      session.insertForm(Object.fromEntries(new window.FormData(event.currentTarget)));
      status.textContent = 'Form inserted. Check its submission URL and field names before using the page.';
    });
  };
  message('booted');
  return () => {
    window.removeEventListener('message', onMessage);
    editor?.destruct(); editor = null; session = null;
  };
}
