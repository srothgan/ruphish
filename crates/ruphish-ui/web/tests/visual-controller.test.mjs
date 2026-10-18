// Real controller and Jodit DOM integration. jsdom does not enforce browser sandbox/CSP.
import {test} from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {JSDOM} from 'jsdom';
import {visualFrameDocument} from '../visual-shell.mjs';
import {installVisualController} from '../visual-controller.mjs';

const vendor = fs.readFileSync(new URL('../node_modules/jodit/es2021/jodit.js', import.meta.url), 'utf8');
function harness(fail = false) {
  const dom = new JSDOM(visualFrameDocument('http://localhost/visual-frame.js', 'http://localhost/visual-frame.css'), {
    url: 'http://localhost/', pretendToBeVisual: true, runScripts: 'outside-only',
  });
  const {window} = dom;
  window.matchMedia = () => ({matches: false, addListener() {}, removeListener() {}});
  window.eval(vendor);
  const messages = [];
  let editor;
  const dispose = installVisualController(window, (element, configuration) => {
    if (fail) throw new window.DOMException('Permission denied to access property "document" on cross-origin object', 'SecurityError');
    const options = window.JSON.parse(JSON.stringify({...configuration, ownerWindow: undefined}));
    options.ownerWindow = window;
    editor = window.Jodit.make(element, options);
    return editor;
  }, message => messages.push(message));
  return {
    window, document: window.document, messages, get editor() { return editor; },
    receive(data, source = window.parent) { window.dispatchEvent(new window.MessageEvent('message', {source, data})); },
    close() { dispose(); window.close(); },
  };
}
const opening = source => ({type: 'open', token: 'session-token', source, variables: ['FirstName', 'URL']});

test('actual host initializes its toolbar, formats content, inserts a field and applies without mutating the supplied source', () => {
  const h = harness();
  const source = '<!doctype html><html><head><style>p{color:#135780}</style></head><body><p>Bonjour José 👩‍💻</p></body></html>';
  try {
    assert.equal(h.messages[0].type, 'booted');
    assert.equal(h.document.querySelector('#editing').hidden, true);
    h.receive(opening(source));
    assert.equal(h.messages.at(-1).type, 'opened');
    assert.equal(h.document.querySelector('#editing').hidden, false);
    assert.equal(h.document.querySelector('#apply').disabled, false);
    assert.ok(h.document.querySelector('[data-ref="bold"]'), 'The actual formatting toolbar must be mounted');
    const paragraph = h.editor.editor.querySelector('p');
    h.editor.s.select(paragraph, true);
    h.editor.execCommand('bold');
    h.editor.s.setCursorIn(h.editor.editor, false);
    const picker = h.document.querySelector('#variable');
    picker.value = 'FirstName'; picker.dispatchEvent(new h.window.Event('change'));
    h.document.querySelector('#apply').click();
    const applied = h.messages.at(-1);
    assert.equal(applied.type, 'apply');
    assert.equal(applied.token, 'session-token');
    const saved = new JSDOM(applied.source).window.document;
    assert.ok(saved.querySelector('strong,b,[style*="font-weight"]'), 'Bold must survive apply');
    assert.ok(saved.body.textContent.includes('Bonjour José 👩‍💻'));
    assert.ok(applied.source.includes('{{.FirstName}}'));
    assert.ok(applied.source.includes('p{color:#135780}'));
    assert.ok(!applied.source.includes('data-ruphish-editor-policy'));
    assert.ok(!source.includes('{{.FirstName}}'));
  } finally { h.close(); }
});

test('SecurityError during initialization exposes a useful failure and cannot apply partial output', () => {
  const h = harness(true);
  try {
    h.receive(opening('<p>Keep this draft</p>'));
    assert.equal(h.messages.at(-1).type, 'failed');
    assert.equal(h.document.querySelector('#editing').hidden, true);
    assert.equal(h.document.querySelector('#apply').disabled, true);
    assert.equal(h.document.querySelector('#variable').disabled, true);
    assert.ok(h.document.querySelector('#variable').textContent.trim());
    assert.match(h.document.querySelector('#status').textContent, /source is unchanged.*Cancel visual edits.*SecurityError/s);
    h.document.querySelector('#apply').click();
    h.receive({type: 'insert', token: 'session-token', name: 'FirstName'});
    assert.equal(h.messages.filter(message => message.type === 'apply').length, 0);
  } finally { h.close(); }
});

test('untrusted messages cannot insert fields; reopening resets the session and no-op apply preserves the new source', () => {
  const h = harness();
  try {
    h.receive(opening('<p>First draft</p>'));
    const original = h.editor;
    h.receive({type: 'insert', token: 'wrong-token', name: 'FirstName'});
    h.receive({type: 'insert', token: 'session-token', name: 'Unlisted'});
    h.receive({type: 'insert', token: 'session-token', name: 'FirstName'}, null);
    h.document.querySelector('#apply').click();
    assert.equal(h.messages.at(-1).source, '<p>First draft</p>');
    h.receive({...opening('<p>Second draft</p>'), token: 'replacement-token'});
    assert.notEqual(h.editor, original);
    h.document.querySelector('#apply').click();
    assert.equal(h.messages.at(-1).token, 'replacement-token');
    assert.equal(h.messages.at(-1).source, '<p>Second draft</p>');
  } finally { h.close(); }
});
