import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { JSDOM } from 'jsdom';

const collector = await readFile(new URL('../../../ruphish-adapters/src/landing-collector.js', import.meta.url), 'utf8');

function page() {
    const dom = new JSDOM(`<form id="training"><input name="username" value="username-canary"><input id="pw" name="password" type="password" value="password-canary"><input name="otp" value="otp-canary"><textarea name="notes">notes-canary</textarea><button name="button" value="button-canary">Continue</button></form><input form="training" type="password" value="associated-canary">`, { url: 'https://training.example.test/', runScripts: 'outside-only' });
    return dom;
}

test('actual collector preserves controls, emits only presence and deduplicates successful submission', async () => {
    const dom = page();
    const { window } = dom;
    const requests = [];
    let received;
    const arrived = new Promise(resolve => { received = resolve; });
    window.fetch = async (url, options) => {
        requests.push({ url, options });
        received();
        return { ok: true, json: async () => ({ success: true, redirect: null }) };
    };
    window.eval(`(()=>{ const endpoint='https://training.example.test/?rid=recipient'; ${collector} })()`);
    window.document.querySelector('#pw').type = 'text'; // Revealing a password retains its designated role.
    const form = window.document.querySelector('form');
    const event = new window.Event('submit', { bubbles: true, cancelable: true });
    form.dispatchEvent(event);
    assert.equal(event.defaultPrevented, true);
    await arrived;
    await new Promise(resolve => setImmediate(resolve));
    form.dispatchEvent(new window.Event('submit', { bubbles: true, cancelable: true }));
    assert.equal(requests.length, 1);
    const { options } = requests[0];
    const payload = JSON.parse(options.body);
    assert.deepEqual(Object.keys(payload).sort(), ['event_id', 'password_present']);
    assert.match(payload.event_id, /^[0-9a-f]{32}$/);
    assert.equal(payload.password_present, true);
    assert.equal(options.credentials, 'omit');
    assert.equal(options.redirect, 'error');
    assert.ok(!JSON.stringify(requests).includes('canary'));
    assert.equal(window.document.querySelector('#pw').name, 'password');
    dom.window.close();
});

test('failed submission retains the same event identity on retry without retaining entered values', async () => {
    const dom = page();
    const { window } = dom;
    const requests = [];
    window.fetch = async (_url, options) => {
        requests.push(JSON.parse(options.body));
        if (requests.length === 1) throw new Error('simulated connection loss');
        return { ok: true, json: async () => ({ success: true, redirect: null }) };
    };
    window.eval(`(()=>{ const endpoint='https://training.example.test/?rid=recipient'; ${collector} })()`);
    const form = window.document.querySelector('form');
    form.dispatchEvent(new window.Event('submit', { bubbles: true, cancelable: true }));
    await new Promise(resolve => setImmediate(resolve));
    assert.match(form.textContent, /Please try again/);
    for (const input of window.document.querySelectorAll('input')) input.value = '';
    form.dispatchEvent(new window.Event('submit', { bubbles: true, cancelable: true }));
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(requests[0], requests[1]);
    assert.equal(requests[1].password_present, true);
    assert.ok(!JSON.stringify(requests).includes('canary'));
    dom.window.close();
});

test('empty controls produce false and ordinary typing produces no network requests', async () => {
    const dom = page();
    const { window } = dom;
    const requests = [];
    window.fetch = async (_url, options) => { requests.push(JSON.parse(options.body)); return { ok: true, json: async () => ({ success: true, redirect: null }) }; };
    window.eval(`(()=>{ const endpoint='https://training.example.test/?rid=recipient'; ${collector} })()`);
    for (const input of window.document.querySelectorAll('input')) {
        input.value = '';
        input.dispatchEvent(new window.Event('input', { bubbles: true }));
    }
    assert.equal(requests.length, 0);
    window.document.querySelector('form').dispatchEvent(new window.Event('submit', { bubbles: true, cancelable: true }));
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(requests.length, 1);
    assert.equal(requests[0].password_present, false);
    dom.window.close();
});

// Drive the actual collector's deadline without wall-clock sleeps. Fetch/body
// doubles reproduce browser AbortSignal rejection at each network boundary.
for (const phase of ['headers', 'body']) {
    test(`a stalled ${phase} response times out and retries the same submission once`, async () => {
        const dom = page();
        try {
            const { window } = dom;
            const timers = new Map();
            let timerId = 0;
            window.setTimeout = (callback, delay) => {
                assert.equal(delay, 15_000);
                timers.set(++timerId, callback);
                return timerId;
            };
            window.clearTimeout = id => timers.delete(id);
            const requests = [];
            const committed = new Set();
            window.fetch = async (_url, options) => {
                const payload = JSON.parse(options.body);
                requests.push({ payload, signal: options.signal });
                committed.add(payload.event_id); // Server committed before its response stalled.
                if (requests.length > 1) return { ok: true, json: async () => ({ success: true, redirect: null }) };
                const stalled = () => new Promise((_, reject) => {
                    options.signal.addEventListener('abort', () => reject(new window.DOMException('Aborted', 'AbortError')), { once: true });
                });
                return phase === 'headers' ? stalled() : { ok: true, json: stalled };
            };
            window.eval(`(()=>{ const endpoint='https://training.example.test/?rid=recipient'; ${collector} })()`);
            const form = window.document.querySelector('form');
            const submit = () => form.dispatchEvent(new window.Event('submit', { bubbles: true, cancelable: true }));
            submit();
            await new Promise(resolve => setImmediate(resolve));
            submit();
            assert.equal(requests.length, 1, 'in-flight repeats must not submit again');
            assert.equal(timers.size, 1);
            timers.values().next().value();
            await new Promise(resolve => setImmediate(resolve));
            assert.equal(requests[0].signal.aborted, true);
            assert.match(form.textContent, /could not be confirmed/);
            assert.equal(timers.size, 0);
            for (const input of window.document.querySelectorAll('input')) input.value = '';
            submit();
            await new Promise(resolve => setImmediate(resolve));
            assert.deepEqual(requests[0].payload, requests[1].payload);
            assert.equal(requests[1].payload.password_present, true);
            assert.equal(committed.size, 1);
            assert.equal(requests[1].signal.aborted, false);
            assert.equal(form.querySelectorAll('[role="status"]').length, 1);
            assert.equal(form.querySelector('[role="status"]').textContent, 'Submitted.');
            assert.equal(timers.size, 0, 'success must cancel its deadline');
            assert.ok(!JSON.stringify(requests).includes('canary'));
            submit();
            assert.equal(requests.length, 2);
        } finally {
            dom.window.close();
        }
    });
}

test('an expired response cannot mark success and an invalid acknowledgement remains retryable', async () => {
    const dom = page();
    try {
        const { window } = dom;
        let expire;
        let completeBody;
        window.setTimeout = callback => { expire = callback; return 1; };
        window.clearTimeout = () => {};
        const requests = [];
        window.fetch = async (_url, options) => {
            requests.push(JSON.parse(options.body));
            if (requests.length === 1) return { ok: true, json: () => new Promise(resolve => { completeBody = resolve; }) };
            return { ok: true, json: async () => ({ success: requests.length > 2, redirect: null }) };
        };
        window.eval(`(()=>{ const endpoint='https://training.example.test/?rid=recipient'; ${collector} })()`);
        const form = window.document.querySelector('form');
        const submit = () => form.dispatchEvent(new window.Event('submit', { bubbles: true, cancelable: true }));
        submit();
        await new Promise(resolve => setImmediate(resolve));
        expire();
        completeBody({ success: true, redirect: null });
        await new Promise(resolve => setImmediate(resolve));
        assert.match(form.textContent, /could not be confirmed/);
        submit();
        await new Promise(resolve => setImmediate(resolve));
        assert.match(form.textContent, /could not be confirmed/);
        submit();
        await new Promise(resolve => setImmediate(resolve));
        assert.equal(form.querySelector('[role="status"]').textContent, 'Submitted.');
        assert.equal(new Set(requests.map(request => request.event_id)).size, 1);
    } finally {
        dom.window.close();
    }
});
