import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile, readdir} from 'node:fs/promises';
import {JSDOM} from 'jsdom';
import {preparePage} from '../starter-templates.mjs';
const messages = await readFile(new URL('../../../ruphish-adapters/src/landing-locales.json', import.meta.url), 'utf8');
const source = await readFile(new URL('../../../ruphish-adapters/src/landing-collector.js', import.meta.url), 'utf8');
const tick = () => new Promise(resolve => setImmediate(resolve));
const ok = () => ({ok: true, json: async () => ({success: true})});
function page(html = '<form id="f"><input name="password" type="password" value="PASSWORD-CANARY"><input name="username" value="USERNAME-CANARY"><textarea name="notes">NOTES-CANARY</textarea><button>Submit</button></form>', setup = () => {}) {
    const dom = new JSDOM(html, {url: 'https://training.example.test/', runScripts: 'outside-only'});
    const requests = [];
    dom.window.fetch = async (url, options) => { requests.push({url, options, payload: JSON.parse(options.body)}); return ok(); };
    setup(dom.window, requests);
    dom.window.eval('(()=>{const endpoint="https://training.example.test/?rid=recipient";const messages=' + messages + ';' + source + '})()');
    return {dom, window: dom.window, requests, form: dom.window.document.querySelector('form')};
}
function submit(p) { p.form.requestSubmit(); }
test('every shipped starter login reports presence only, including prefill and company-logo copies', async () => {
    const root = new URL('../../../ruphish-http/starter-templates/landing-pages/', import.meta.url);
    const parser = new JSDOM('');
    globalThis.DOMParser = parser.window.DOMParser;
    const brands = new Map([
        ['microsoft-full-screen','.microsoft'], ['microsoft-side-panel','.microsoft'],
        ['zoom','.masthead'], ['salesforce','img.logo'], ['workday','.brand'],
    ]);
    try {
        for (const slug of await readdir(root)) {
            const html = await readFile(new URL(`${slug}/landing.html`, root), 'utf8');
            const variants = [html, preparePage(html, true, '', '')];
            if (brands.has(slug)) variants.push(preparePage(html, true,
                JSON.stringify({selector:brands.get(slug),height:36}), 'data:image/png;base64,AAAA'));
            for (const variant of variants) {
                const p = page(variant);
                try {
                    p.form.querySelector('input[type=email]').value = 'USERNAME-CANARY@example.test';
                    p.form.querySelector('input[type=password]').value = 'PASSWORD-CANARY';
                    submit(p); await tick();
                    assert.equal(p.requests.length, 1, slug);
                    assert.equal(p.requests[0].payload.password_present, true, slug);
                    safe(p.requests);
                } finally { p.dom.window.close(); }
            }
        }
    } finally { parser.window.close(); delete globalThis.DOMParser; }
});
function safe(requests) {
    assert.ok(!JSON.stringify(requests).includes('CANARY'));
    for (const {payload, options} of requests) {
        assert.deepEqual(Object.keys(payload).sort(), ['event_id', 'password_present']);
        assert.match(payload.event_id, /^[0-9a-f]{32}$/);
        assert.equal(options.credentials, 'omit');
        assert.equal(options.redirect, 'error');
    }
}
test('three distinct submissions on one form produce three independent observations, including while delivery is pending', async () => {
    const completions = [];
    const p = page(undefined, (w, requests) => { w.fetch = (_url, options) => {
        requests.push({options, payload: JSON.parse(options.body)});
        return new Promise(resolve => completions.push(resolve));
    }; });
    try {
        submit(p);
        p.form.querySelector('input').value = '';
        submit(p);
        assert.equal(p.requests.length, 2);
        completions[1](ok()); completions[0](ok()); await tick();
        p.form.querySelector('input').value = 'AUTOFILL-CANARY';
        submit(p); completions[2](ok()); await tick();
        assert.deepEqual(p.requests.map(r => r.payload.password_present), [true, false, true]);
        assert.equal(new Set(p.requests.map(r => r.payload.event_id)).size, 3);
        assert.equal(p.form.querySelectorAll('[role="status"]').length, 3);
        safe(p.requests);
    } finally { p.dom.window.close(); }
});
test('retrying an unconfirmed observation preserves identity and evidence; submitting again creates a new observation', async () => {
    const p = page(undefined, (w, requests) => { w.fetch = async (_url, options) => {
        requests.push({options, payload: JSON.parse(options.body)});
        if (requests.length === 1) throw new Error('lost acknowledgement');
        return ok();
    }; });
    try {
        submit(p); await tick();
        p.form.querySelector('input').value = '';
        p.form.querySelector('[role="status"] button').click(); await tick();
        assert.deepEqual(p.requests[0].payload, p.requests[1].payload);
        submit(p); await tick();
        assert.equal(p.requests[2].payload.password_present, false);
        assert.notEqual(p.requests[2].payload.event_id, p.requests[0].payload.event_id);
        safe(p.requests);
    } finally { p.dom.window.close(); }
});
test('form submissions respect enabled, named, associated password controls and preserve autofill, readonly and reveal behavior', async () => {
    const cases = [
        ['<input type="password" name="p" value="PREFILL-CANARY">', true],
        ['<input type="password" name="p" readonly value="READONLY-CANARY">', true],
        ['<input type="password" name="p" disabled value="DISABLED-CANARY">', false],
        ['<fieldset disabled><input type="password" name="p" value="FIELDSET-CANARY"></fieldset>', false],
        ['<fieldset disabled><legend><input type="password" name="p" value="LEGEND-CANARY"></legend></fieldset>', true],
        ['<fieldset disabled><legend>First</legend><legend><input type="password" name="p" value="SECOND-CANARY"></legend></fieldset>', false],
        ['<input type="password" value="UNNAMED-CANARY">', false],
        ['<input type="text" name="password" value="TEXT-CANARY">', false],
        ['<input type="password" name="p" value="">', false],
        ['<input name="username" value="USER-CANARY">', false],
        ['<input type="password" name="p" form="other" value="OTHER-CANARY">', false],
    ];
    for (const [controls, expected] of cases) {
        const p = page('<form id="f">'+controls+'<button>Submit</button></form>');
        try { submit(p); await tick(); assert.equal(p.requests[0].payload.password_present, expected, controls); safe(p.requests); }
        finally { p.dom.window.close(); }
    }
    const p = page('<form id="f"><button>Submit</button></form><input form="f" name="p" type="password">');
    try {
        const control = p.window.document.querySelector('input');
        control.value = 'MANAGER-CANARY'; control.type = 'text';
        submit(p); await tick(); assert.equal(p.requests[0].payload.password_present, true); safe(p.requests);
    } finally { p.dom.window.close(); }
});
test('typing and failed native validation do not create submissions; dynamically added password controls are observed', async () => {
    const p = page('<form id="f"><input required name="email" type="email"><button>Submit</button></form>');
    try {
        submit(p); await tick(); assert.equal(p.requests.length, 0);
        p.form.querySelector('input').value = 'synthetic@example.test';
        p.form.querySelector('input').dispatchEvent(new p.window.Event('input', {bubbles:true}));
        const password = p.window.document.createElement('input');
        password.type='password'; password.name='p'; password.value='DYNAMIC-CANARY'; p.form.append(password);
        await tick(); password.type='text';
        assert.equal(p.requests.length, 0);
        submit(p); await tick(); assert.equal(p.requests[0].payload.password_present,true); safe(p.requests);
    } finally { p.dom.window.close(); }
});
for (const phase of ['headers','body']) test('stalled '+phase+' delivery can be retried without duplicating its identity', async () => {
    let expire;
    const p = page(undefined, (w,requests) => {
        w.setTimeout = callback => { expire=callback; return 1; }; w.clearTimeout=()=>{};
        w.fetch=async (_url,options)=>{
            requests.push({options,payload:JSON.parse(options.body)});
            if(requests.length>1)return ok();
            const stalled=()=>new Promise((_,reject)=>options.signal.addEventListener('abort',()=>reject(new Error('timeout'))));
            return phase==='headers'?stalled():{ok:true,json:stalled};
        };
    });
    try {
        submit(p); await tick(); expire(); await tick();
        const retry=p.form.querySelector('[role="status"] button'); assert.equal(retry.hidden,false);
        retry.click(); retry.click(); await tick();
        assert.equal(p.requests.length,2); assert.deepEqual(p.requests[0].payload,p.requests[1].payload);
        assert.equal(p.form.querySelector('[role="status"] span').textContent,'Submitted.'); safe(p.requests);
    } finally { p.dom.window.close(); }
});
test('an acknowledgement arriving after the deadline remains unconfirmed until retried', async () => {
    let expire, acknowledge;
    const p = page(undefined, (window, requests) => {
        window.setTimeout = callback => { expire = callback; return 1; };
        window.clearTimeout = () => {};
        window.fetch = async (_url, options) => {
            requests.push({options, payload: JSON.parse(options.body)});
            return requests.length === 1
                ? {ok: true, json: () => new Promise(resolve => { acknowledge = resolve; })}
                : ok();
        };
    });
    try {
        submit(p); await tick(); expire(); acknowledge({success: true}); await tick();
        const retry = p.form.querySelector('[role="status"] button');
        assert.equal(retry.hidden, false);
        assert.notEqual(p.form.querySelector('[role="status"] span').textContent, 'Submitted.');
        retry.click(); await tick();
        assert.deepEqual(p.requests[0].payload, p.requests[1].payload);
        assert.equal(p.form.querySelector('[role="status"] span').textContent, 'Submitted.');
    } finally { p.dom.window.close(); }
});

test('an invalid acknowledgement remains unconfirmed and retryable',async()=>{
    const p=page(undefined,(w,requests)=>{w.fetch=async(_url,options)=>{requests.push({options,payload:JSON.parse(options.body)});return {ok:true,json:async()=>({success:requests.length>1})};};});
    try { submit(p);await tick();const retry=p.form.querySelector('[role="status"] button');assert.equal(retry.hidden,false);retry.click();await tick();assert.deepEqual(p.requests[0].payload,p.requests[1].payload); }
    finally { p.dom.window.close(); }
});
test('submission feedback follows document and browser language without changing authored controls',async()=>{
    for(const [lang,languages,expected]of [['de-DE',['en'],'Übermittelt.'],['en',['de'],'Submitted.'],['',['de'],'Übermittelt.'],['fr',['zh'],'Submitted.']]){
        const p=page(undefined,w=>{w.document.documentElement.lang=lang;Object.defineProperty(w.navigator,'languages',{value:languages});});
        try {submit(p);await tick();assert.equal(p.form.querySelector('[role="status"] span').textContent,expected);assert.equal(p.form.querySelector('input').value,'PASSWORD-CANARY');}
        finally{p.dom.window.close();}
    }
});
