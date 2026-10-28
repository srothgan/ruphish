// This code is injected only after authored content has been processed.
// No FormData, field maps, entered values or value-derived strings cross the network.
const passwordControls = new WeakSet(document.querySelectorAll('input[type="password"]'));
// Remember password roles across reveal toggles, including newly inserted controls.
const remember = (root) => {
  if (root instanceof HTMLInputElement && root.type === 'password') {
    passwordControls.add(root);
  }
  root
    .querySelectorAll?.('input[type="password"]')
    .forEach((control) => passwordControls.add(control));
};
new MutationObserver((records) =>
  records.forEach((record) => {
    remember(record.target);
    record.addedNodes.forEach(remember);
  }),
).observe(document, {
  subtree: true,
  childList: true,
  attributes: true,
  attributeFilter: ['type'],
});
const pending = new Set();
let destination = null;
const candidates = [
  document.documentElement.lang,
  ...(navigator.languages || [navigator.language]),
];
const language =
  candidates
    .map((value) => (value || '').split('-')[0].toLowerCase())
    .find((value) => Object.hasOwn(messages, value)) || 'en';
const text = messages[language];
document.addEventListener(
  'submit',
  (event) => {
    event.preventDefault();
    const form = event.target;
    if (!(form instanceof HTMLFormElement)) {
      return;
    }
    const notice = document.createElement('p');
    notice.setAttribute('role', 'status');
    const status = document.createElement('span');
    const retry = document.createElement('button');
    retry.type = 'button';
    retry.textContent = text.retry;
    retry.hidden = true;
    notice.append(status, retry);
    form.append(notice);
    try {
      const bytes = crypto.getRandomValues(new Uint8Array(16));
      const event_id = Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
      const password_present = Array.from(form.elements).some(
        (control) =>
          control instanceof HTMLInputElement &&
          (passwordControls.has(control) || control.type === 'password') &&
          !control.matches(':disabled') &&
          control.name !== '' &&
          !control.closest('datalist') &&
          control.value !== '',
      );
      const attempt = {
        payload: { event_id, password_present },
        status,
        retry,
        busy: false,
        done: false,
      };
      pending.add(attempt);
      retry.addEventListener('click', () => deliver(attempt));
      void deliver(attempt);
    } catch {
      status.textContent = text.unconfirmed;
    }
  },
  true,
);

async function deliver(attempt) {
  if (attempt.busy || attempt.done) {
    return;
  }
  attempt.busy = true;
  attempt.retry.hidden = true;
  attempt.status.textContent = text.submitting;
  // Cover both response headers and body, allowing time beyond the server's 10s
  // deadline. A timeout means unconfirmed, not necessarily unrecorded.
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 15_000);
  try {
    const response = await fetch(endpoint, {
      method: 'POST',
      credentials: 'omit',
      redirect: 'error',
      cache: 'no-store',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(attempt.payload),
      signal: controller.signal,
    });
    if (!response.ok) {
      throw new Error('submission_failed');
    }
    const result = await response.json();
    if (controller.signal.aborted || result?.success !== true) {
      throw new Error('submission_unconfirmed');
    }
    attempt.done = true;
    pending.delete(attempt);
    attempt.status.textContent = text.submitted;
    if (typeof result.redirect === 'string') {
      destination = result.redirect;
    }
    // Do not abandon another observed submission merely because this one finished first.
    if (pending.size === 0 && destination !== null) {
      location.assign(destination);
    }
  } catch {
    attempt.status.textContent = text.unconfirmed;
    attempt.retry.hidden = false;
  } finally {
    clearTimeout(timeout);
    attempt.busy = false;
  }
}
