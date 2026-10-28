// Real controller and Jodit DOM integration. jsdom does not enforce browser sandbox/CSP.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import german from 'jodit/esm/langs/de.js';
import { JSDOM } from 'jsdom';
import { visualFrameDocument } from '../visual-shell.mjs';
import { installVisualController } from '../visual-controller.mjs';

const vendor = fs.readFileSync(
  new URL('../node_modules/jodit/es2021/jodit.js', import.meta.url),
  'utf8',
);
function harness(fail = false, allowForms = true) {
  const dom = new JSDOM(
    visualFrameDocument(
      'http://localhost/visual-frame.js',
      'http://localhost/visual-frame.css',
      'en',
      allowForms,
    ),
    {
      url: 'http://localhost/',
      pretendToBeVisual: true,
      runScripts: 'outside-only',
    },
  );
  const { window } = dom;
  window.matchMedia = () => ({ matches: false, addListener() {}, removeListener() {} });
  window.eval(vendor);
  const messages = [];
  let editor;
  const dispose = installVisualController(
    window,
    (element, configuration) => {
      if (fail) {
        throw new window.DOMException(
          'Permission denied to access property "document" on cross-origin object',
          'SecurityError',
        );
      }
      const options = window.JSON.parse(
        JSON.stringify({ ...configuration, ownerWindow: undefined }),
      );
      options.ownerWindow = window;
      options.i18n = { de: window.JSON.parse(JSON.stringify(german)) };
      editor = window.Jodit.make(element, options);
      return editor;
    },
    (message) => messages.push(message),
  );
  return {
    window,
    document: window.document,
    messages,
    get editor() {
      return editor;
    },
    receive(data, source = window.parent) {
      window.dispatchEvent(new window.MessageEvent('message', { source, data }));
    },
    close() {
      dispose();
      window.close();
    },
  };
}
const opening = (source) => ({
  type: 'open',
  token: 'session-token',
  source,
  variables: ['FirstName', 'URL'],
});

test('changing the UI language preserves the visual session, authored text and unsaved form values', () => {
  const h = harness();
  const source =
    '<html lang="fr"><body><p data-editor-text="apply">Author text: Apply visual changes</p></body></html>';
  try {
    h.receive({ ...opening(source), locale: 'de' });
    const editor = h.editor;
    assert.equal(editor.o.language, 'de');
    assert.equal(editor.i18n('Bold'), 'Fett');
    const input = h.document.querySelector('#form').elements.namedItem('label');
    input.value = 'My unsaved field label';
    editor.editor.querySelector('p').textContent = 'Unsaved author content';
    const before = editor.value;
    h.receive({ type: 'locale', token: 'wrong-token', locale: 'en' });
    assert.equal(h.document.documentElement.lang, 'de');
    h.receive({ type: 'locale', token: 'session-token', locale: 'en' });
    assert.equal(h.editor, editor);
    assert.equal(editor.value, before);
    assert.equal(input.value, 'My unsaved field label');
    assert.equal(h.document.documentElement.lang, 'en');
    assert.equal(editor.ed.documentElement.lang, 'fr');
    h.receive({ type: 'locale', token: 'session-token', locale: 'de' });
    assert.equal(editor.value, before);
  } finally {
    h.close();
  }
});

test('visual edits and recipient fields synchronize without mutating the supplied source', () => {
  const h = harness();
  const source =
    '<!doctype html><html><head><style>p{color:#135780}</style></head><body><p>Bonjour José 👩‍💻</p></body></html>';
  try {
    assert.equal(h.messages[0].type, 'booted');
    assert.equal(h.document.querySelector('#editing').hidden, true);
    h.receive(opening(source));
    assert.equal(h.messages.at(-1).type, 'opened');
    assert.equal(h.document.querySelector('#editing').hidden, false);
    assert.ok(
      h.document.querySelector('[data-ref="bold"]'),
      'The actual formatting toolbar must be mounted',
    );
    const paragraph = h.editor.editor.querySelector('p');
    h.editor.s.select(paragraph, true);
    h.editor.execCommand('bold');
    h.editor.s.setCursorIn(h.editor.editor, false);
    h.receive({ type: 'insert', token: 'session-token', name: 'FirstName' });
    h.receive({ type: 'snapshot', token: 'session-token' });
    const applied = h.messages.at(-1);
    assert.equal(applied.type, 'snapshot');
    assert.equal(applied.token, 'session-token');
    const saved = new JSDOM(applied.source).window.document;
    assert.ok(saved.querySelector('strong,b,[style*="font-weight"]'), 'Bold must survive apply');
    assert.ok(saved.body.textContent.includes('Bonjour José 👩‍💻'));
    assert.ok(applied.source.includes('{{.FirstName}}'));
    assert.ok(applied.source.includes('p{color:#135780}'));
    assert.ok(!applied.source.includes('data-ruphish-editor-policy'));
    assert.ok(!source.includes('{{.FirstName}}'));
  } finally {
    h.close();
  }
});

test('startup failure cannot publish partial output', () => {
  const h = harness(true);
  try {
    h.receive(opening('<p>Keep this draft</p>'));
    assert.equal(h.messages.at(-1).type, 'failed');
    assert.equal(h.document.querySelector('#editing').hidden, true);
    assert.match(
      h.document.querySelector('#status').textContent,
      /edits are retained.*SecurityError/s,
    );
    h.receive({ type: 'snapshot', token: 'session-token' });
    h.receive({ type: 'insert', token: 'session-token', name: 'FirstName' });
    assert.equal(h.messages.filter((message) => message.type === 'snapshot').length, 0);
  } finally {
    h.close();
  }
});

test('untrusted messages cannot insert fields; reopening and no-op snapshots preserve source', () => {
  const h = harness();
  try {
    h.receive(opening('<p>First draft</p>'));
    const original = h.editor;
    h.receive({ type: 'insert', token: 'wrong-token', name: 'FirstName' });
    h.receive({ type: 'insert', token: 'session-token', name: 'Unlisted' });
    h.receive({ type: 'insert', token: 'session-token', name: 'FirstName' }, null);
    h.receive({ type: 'snapshot', token: 'session-token' });
    assert.equal(h.messages.at(-1).source, '<p>First draft</p>');
    h.receive({ ...opening('<p>Second draft</p>'), token: 'replacement-token' });
    assert.notEqual(h.editor, original);
    h.receive({ type: 'snapshot', token: 'replacement-token' });
    assert.equal(h.messages.at(-1).token, 'replacement-token');
    assert.equal(h.messages.at(-1).source, '<p>Second draft</p>');
  } finally {
    h.close();
  }
});

test('email editing has no form controls and synchronizes input without an apply action', () => {
  const h = harness(false, false);
  try {
    h.receive(opening('<p>Original</p>'));
    assert.equal(h.document.querySelector('#form'), null);
    assert.equal(h.document.querySelector('#load-field'), null);
    assert.equal(h.document.querySelector('#apply'), null);
    assert.equal(h.document.querySelector('#variable'), null);
    h.editor.editor.querySelector('p').textContent = 'Changed';
    h.editor.editor.dispatchEvent(new h.window.Event('input', { bubbles: true }));
    assert.equal(h.messages.at(-1).type, 'change');
    assert.match(h.messages.at(-1).source, /Changed/);
  } finally {
    h.close();
  }
});
