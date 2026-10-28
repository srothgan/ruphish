import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { JSDOM } from 'jsdom';
import { preparePage } from '../starter-templates.mjs';

const root = new URL('../../../ruphish-http/starter-templates/', import.meta.url);
const interaction = await readFile(
  new URL('../../../ruphish-http/src/starter_templates/interaction.js', import.meta.url),
  'utf8',
);
const collector = await readFile(
  new URL('../../../ruphish-adapters/src/landing-collector.js', import.meta.url),
  'utf8',
);
const messages = await readFile(
  new URL('../../../ruphish-adapters/src/landing-locales.json', import.meta.url),
  'utf8',
);
const tick = () => new Promise((resolve) => setImmediate(resolve));
const slugs = [
  'onedrive-shared-document',
  'google-drive-shared-document',
  'dropbox-shared-file',
  'microsoft-two-step',
  'microsoft-two-step-side-panel',
  'google-workspace-two-step',
];

async function fixture(slug, preview = false, prefill = false) {
  let html = await readFile(new URL(`landing-pages/${slug}/landing.html`, root), 'utf8');
  if (prefill) {
    const parser = new JSDOM('');
    globalThis.DOMParser = parser.window.DOMParser;
    try {
      html = preparePage(html, true, '', '').replace('{{.Email | html}}', 'alice@example.test');
    } finally {
      delete globalThis.DOMParser;
      parser.window.close();
    }
  }
  // Execute the actual inline ordering used by live rendering: collector before page script.
  html = html
    .replace(
      '<head>',
      `<head><script>(()=>{const endpoint=${preview ? 'null' : '"https://exercise.test/?rid=synthetic"'};const messages=${messages};${collector}})();</script>`,
    )
    .replace('</body>', `<script>${interaction}</script></body>`);
  const requests = [];
  let acknowledge;
  const dom = new JSDOM(html, {
    url: 'https://exercise.test/',
    runScripts: 'dangerously',
    beforeParse(window) {
      window.fetch = async (_url, options) => {
        requests.push(JSON.parse(options.body));
        return new Promise((resolve) => {
          acknowledge = (success = true) =>
            resolve({ ok: success, json: async () => ({ success }) });
        });
      };
    },
  });
  const document = dom.window.document;
  return {
    dom,
    document,
    requests,
    form: document.querySelector('form'),
    acknowledge: (...args) => acknowledge(...args),
  };
}

for (const slug of slugs) {
  test(`${slug}: local navigation, required fields, reveal, and one presence-only final observation`, async () => {
    const f = await fixture(slug);
    try {
      const { document, form, requests } = f;
      const username = form.elements.namedItem('username');
      const password = form.elements.namedItem('password');
      const open = document.querySelector('[data-open]');
      if (open) {
        assert.equal(document.querySelector('[data-login]').hidden, true);
        assert.equal(username.disabled || username.closest('fieldset').disabled, true);
        open.click();
      }
      form.requestSubmit();
      assert.equal(requests.length, 0, 'empty required fields do not submit');
      username.value = 'not-an-email';
      form.requestSubmit();
      assert.equal(requests.length, 0);
      username.value = 'USERNAME-CANARY@example.test';
      const staged = slug !== 'dropbox-shared-file';
      if (staged) {
        form.requestSubmit();
        assert.equal(requests.length, 0, 'Next is not an observation');
        assert.equal(document.querySelector('[data-account]').textContent, username.value);
        assert.equal(document.activeElement, password);
        assert.equal(document.querySelector('[data-identifier]').hidden, true);
        password.value = 'DISCARDED-CANARY';
        const reveal = document.querySelector('[data-reveal]');
        reveal.click();
        assert.equal(password.type, 'text');
        document.querySelector('[data-back]:not([data-back-file])').click();
        assert.equal(password.value, '', 'Back clears the password');
        assert.equal(password.type, 'password');
        assert.equal(username.value, 'USERNAME-CANARY@example.test');
        assert.equal(document.activeElement, username);
        form.requestSubmit();
      }
      form.requestSubmit();
      assert.equal(requests.length, 0, 'empty password does not submit');
      password.value = 'PASSWORD-CANARY';
      document.querySelector('[data-reveal]').click();
      const defaultSubmit = form.querySelector('button[type="submit"]');
      assert.equal(
        defaultSubmit.matches(':disabled'),
        false,
        'Enter has an enabled default action',
      );
      assert.equal(defaultSubmit, document.querySelector('[data-final]'));
      defaultSubmit.click();
      form.requestSubmit();
      assert.equal(requests.length, 1, 'double activation cannot create another observation');
      assert.deepEqual(Object.keys(requests[0]).sort(), ['event_id', 'password_present']);
      assert.equal(requests[0].password_present, true);
      assert.doesNotMatch(JSON.stringify(requests), /CANARY/);
      assert.equal(document.querySelector('[data-final]').disabled, true);
      f.acknowledge();
      await tick();
      assert.match(form.textContent, /Submitted\./);
      form.requestSubmit();
      assert.equal(requests.length, 1, 'acknowledged submission stays complete');
    } finally {
      f.dom.window.close();
    }
  });
}

test('prefilled previews remain local and returning to the shared file clears the password', async () => {
  for (const slug of slugs) {
    const f = await fixture(slug, true, true);
    try {
      f.document.querySelector('[data-open]')?.click();
      assert.equal(f.form.elements.namedItem('username').value, 'alice@example.test');
      if (slug !== 'dropbox-shared-file') {
        f.form.requestSubmit();
        assert.equal(f.document.querySelector('[data-account]').textContent, 'alice@example.test');
      }
      const back = f.document.querySelector('[data-back-file]');
      if (back) {
        f.form.elements.namedItem('password').value = 'DISCARDED-PREVIEW-CANARY';
        back.click();
        assert.equal(f.document.querySelector('[data-sharing]').hidden, false);
        assert.equal(f.form.elements.namedItem('password').value, '');
        f.document.querySelector('[data-open]').click();
        if (slug !== 'dropbox-shared-file') {
          f.form.requestSubmit();
        }
      }
      f.form.elements.namedItem('password').value = 'PREVIEW-CANARY';
      f.form.requestSubmit();
      await tick();
      assert.equal(f.requests.length, 0);
      assert.match(f.form.textContent, /Preview/);
      if (back) {
        assert.equal(back.disabled, true, 'completed preview stays complete until reopened');
      }
    } finally {
      f.dom.window.close();
    }
  }
});

test('uncertain final delivery uses the collector retry without creating another event', async () => {
  const f = await fixture('microsoft-two-step');
  try {
    f.form.elements.namedItem('username').value = 'alice@example.test';
    f.form.requestSubmit();
    f.form.elements.namedItem('password').value = 'SECRET-CANARY';
    f.form.requestSubmit();
    f.acknowledge(false);
    await tick();
    assert.equal(f.document.querySelector('[data-final]').disabled, true);
    const retry = [...f.form.querySelectorAll('button')].find(
      (button) => button.textContent === 'Retry this submission',
    );
    assert.ok(retry && !retry.hidden);
    retry.click();
    assert.equal(f.requests.length, 2);
    assert.deepEqual(f.requests[1], f.requests[0]);
    f.acknowledge();
    await tick();
    assert.match(f.form.textContent, /Submitted\./);
  } finally {
    f.dom.window.close();
  }
});
