// Invoked by the real executable workflow with its isolated HTTPS listener and CA.
// Executes the actual served collector; assertions also inspect every outgoing body.
import assert from 'node:assert/strict';
import https from 'node:https';
import {readFile} from 'node:fs/promises';
import {JSDOM} from 'jsdom';

const [url, certificate] = process.argv.slice(2);
const ca = await readFile(certificate);
function request(target, options = {}) {
    assert.equal(new URL(target).origin, new URL(url).origin);
    return new Promise((resolve, reject) => {
        const outgoing = https.request(target, {
            method: options.method || 'GET', headers: options.headers, ca,
            lookup: (_host, options, done) => done(null, options.all ? [{address: '127.0.0.1', family: 4}] : '127.0.0.1', 4),
        }, response => {
            const chunks = [];
            response.on('data', chunk => chunks.push(chunk));
            response.on('end', () => resolve({ok: response.statusCode === 200, text: Buffer.concat(chunks).toString()}));
        });
        outgoing.on('error', reject);
        outgoing.setTimeout(10000, () => outgoing.destroy(new Error('fixture request timeout')));
        outgoing.end(options.body);
    });
}
const loaded = await request(url);
assert.equal(loaded.ok, true);
const dom = new JSDOM(loaded.text, {url, runScripts: 'outside-only'});
try {
    const requests = [];
    let delivered;
    let lost = false;
    dom.window.fetch = async (target, options) => {
        assert.ok(!options.body.includes('CANARY'));
        requests.push(JSON.parse(options.body));
        const response = await request(target, options);
        delivered?.();
        if (lost) { lost = false; throw new Error('lost acknowledgement after commit'); }
        return {ok: response.ok, json: async () => JSON.parse(response.text)};
    };
    dom.window.eval(dom.window.document.querySelector('script[nonce]').textContent);
    const form = dom.window.document.querySelector('form');
    const password = form.querySelector('input[type="password"]');
    const submit = async () => {
        const arrived = new Promise(resolve => { delivered = resolve; });
        form.requestSubmit();
        await arrived;
        await new Promise(resolve => setImmediate(resolve));
    };
    password.value = 'AUTOFILL-CANARY';
    await submit();
    password.disabled = true;
    await submit();
    password.disabled = false;
    lost = true;
    await submit();
    const arrived = new Promise(resolve => { delivered = resolve; });
    form.querySelectorAll('[role="status"] button')[2].click();
    await arrived;
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(requests.length, 4);
    assert.equal(new Set(requests.map(event => event.event_id)).size, 3);
    assert.deepEqual(requests.map(event => event.password_present), [true, false, true, true]);
    assert.deepEqual(requests[2], requests[3]);
    assert.equal(form.querySelectorAll('[role="status"] span')[2].textContent, 'Submitted.');
} finally {
    dom.window.close();
}
