// This code is injected only after authored content has been processed.
// No FormData, field maps, entered values or value-derived strings cross the network.
const passwordControls = new WeakSet(document.querySelectorAll('input[type="password"]'));
const pending = new WeakMap();
document.addEventListener('submit', async (event) => {
    event.preventDefault();
    const form = event.target;
    if (!(form instanceof HTMLFormElement)) return;
    let attempt = pending.get(form);
    if (attempt?.busy || attempt?.done) return;
    if (!attempt) {
        const bytes = crypto.getRandomValues(new Uint8Array(16));
        const event_id = Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('');
        const password_present = Array.from(form.elements).some(control =>
            control instanceof HTMLInputElement && passwordControls.has(control) && control.value !== '');
        const notice = document.createElement('p');
        notice.setAttribute('role', 'status');
        form.append(notice);
        attempt = { payload: { event_id, password_present }, notice, busy: false, done: false };
        pending.set(form, attempt);
    }
    attempt.busy = true;
    const notice = attempt.notice;
    notice.textContent = 'Submitting…';
    // Cover both response headers and body, allowing time beyond the server's 10s
    // deadline. A timeout means unconfirmed, not necessarily unrecorded.
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 15_000);
    try {
        const response = await fetch(endpoint, {
            method: 'POST', credentials: 'omit', redirect: 'error', cache: 'no-store',
            headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(attempt.payload),
            signal: controller.signal
        });
        if (!response.ok) throw new Error('submission_failed');
        const result = await response.json();
        if (controller.signal.aborted || result?.success !== true) throw new Error('submission_unconfirmed');
        attempt.done = true;
        notice.textContent = 'Submitted.';
        if (typeof result.redirect === 'string') location.assign(result.redirect);
    } catch {
        notice.textContent = 'Submission could not be confirmed. Please try again.';
    } finally {
        clearTimeout(timeout);
        attempt.busy = false;
    }
}, true);
