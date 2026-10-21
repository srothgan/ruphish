// Real editor DOM/model APIs under jsdom. Browser input/layout and sandbox enforcement are manual checks.
import {test} from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {JSDOM} from 'jsdom';
import {undo, redo} from '@codemirror/commands';
import {VisualSession, visualOptions} from '../visual-session.mjs';
import {TemplateVault} from '../template-html.mjs';
import {createState, applyVisual} from '../state.mjs';

const vendor = fs.readFileSync(new URL('../node_modules/jodit/es2021/jodit.js', import.meta.url), 'utf8');
const fixture = JSON.parse(fs.readFileSync(new URL('../../../../tests/fixtures/compatibility/editor-template.json', import.meta.url), 'utf8'));
function editor() {
  const dom = new JSDOM('<!doctype html><textarea id="editor"></textarea>', {url: 'http://localhost/', pretendToBeVisual: true, runScripts: 'outside-only'});
  dom.window.matchMedia = () => ({matches: false, addListener() {}, removeListener() {}});
  dom.window.eval(vendor);
  const options = visualOptions(dom.window);
  delete options.ownerWindow;
  const realmOptions = dom.window.JSON.parse(JSON.stringify({...options, toolbar: false}));
  realmOptions.ownerWindow = dom.window;
  const value = dom.window.Jodit.make(dom.window.document.querySelector('#editor'), realmOptions);
  return {value, close() { value.destruct(); dom.window.close(); }};
}

test('open/cancel/no-op apply retain original source, while an actual visual edit survives one source undo/redo transaction', () => {
  const harness = editor();
  try {
    const session = new VisualSession(harness.value, fixture.html);
    assert.equal(session.result(), fixture.html);
    assert.equal(applyVisual(createState(fixture.html), fixture.html, session.result()), null);
    const paragraph = harness.value.editor.querySelector('p');
    paragraph.prepend(harness.value.ed.createTextNode('Welcome '));
    const edited = session.result();
    assert.notEqual(edited, fixture.html);
    assert.ok(edited.includes('Welcome Hello'));
    assert.ok(edited.toLowerCase().startsWith('<!doctype html>'));
    assert.ok(edited.includes('p{color:#135780}'));
    assert.ok(edited.includes('{{.FirstName}}') && edited.includes('{{.URL}}'));
    let state = createState(fixture.html);
    const target = {get state() { return state; }, dispatch(transaction) { state = transaction.state; }};
    target.dispatch(applyVisual(state, fixture.html, edited));
    const saved = JSON.parse(JSON.stringify({...fixture, html: state.sliceDoc()}));
    assert.deepEqual(saved.attachments, fixture.attachments);
    assert.ok(undo(target)); assert.equal(state.sliceDoc(), fixture.html);
    assert.ok(redo(target)); assert.equal(state.sliceDoc(), edited);
    assert.throws(() => applyVisual(createState('newer source'), fixture.html, edited), /source changed/);
  } finally { harness.close(); }
});

test('actual visual insertion retains form behavior, names, Unicode labels and variables', () => {
  const harness = editor();
  try {
    const session = new VisualSession(harness.value, '<p>Introduction</p>');
    harness.value.s.setCursorIn(harness.value.editor, false);
    session.insertForm({action: '{{.URL}}', method: 'post', name: 'email', label: 'Email – Grüße', type: 'email', submitLabel: 'Weiter'});
    session.insertVariable('FirstName');
    const source = session.result();
    assert.ok(!source.includes('data-ruphish-editor-policy'));
    assert.ok(!source.includes("default-src 'none'"));
    const document = new JSDOM(source).window.document;
    assert.equal(document.querySelector('form').getAttribute('action'), '{{.URL}}');
    assert.equal(document.querySelector('form').getAttribute('method'), 'post');
    assert.equal(document.querySelector('input').name, 'email');
    assert.equal(document.querySelector('input').type, 'email');
    assert.equal(document.querySelector('button').type, 'submit');
    assert.equal(document.querySelector('button').textContent, 'Weiter');
    assert.ok(document.querySelector('label').textContent.includes('Grüße'));
    assert.ok(source.includes('{{.FirstName}}'));
    assert.ok(!source.includes('data-ruphish-field'));
    const field = harness.value.editor.querySelector('input');
    session.selectField(field);
    assert.equal(session.selectedProperties().action, '{{.URL}}');
    session.updateSelected({action: '{{.BaseURL}}/continue', method: 'get', name: 'login', label: 'Adresse', type: 'text'});
    const changed = new JSDOM(session.result()).window.document;
    assert.equal(changed.querySelector('form').getAttribute('action'), '{{.BaseURL}}/continue');
    assert.equal(changed.querySelector('form').getAttribute('method'), 'get');
    assert.equal(changed.querySelector('input').getAttribute('name'), 'login');
    assert.equal(changed.querySelector('input').getAttribute('type'), 'text');
    assert.ok(changed.querySelector('label').textContent.includes('Adresse'));
    assert.equal(changed.querySelector('button').getAttribute('type'), 'submit');
  } finally { harness.close(); }
});

test('editor-only policy never leaks into saved pages, while authored policies and similarly named attributes remain', () => {
  const harness = editor();
  try {
    const fragment = new VisualSession(harness.value, '<p>Fragment</p>');
    harness.value.editor.querySelector('p').append(' edited');
    assert.ok(!fragment.result().includes('data-ruphish-editor-policy'));
    const page = `<html><head><meta http-equiv="Content-Security-Policy" content="img-src 'none'"></head><body><p data-ruphish-editor-policy="authored">Text</p></body></html>`;
    const session = new VisualSession(harness.value, page);
    harness.value.editor.querySelector('p').append(' edited');
    const saved = new JSDOM(session.result()).window.document;
    assert.equal(saved.querySelector('meta[http-equiv]').getAttribute('content'), "img-src 'none'");
    assert.equal(saved.querySelector('p').getAttribute('data-ruphish-editor-policy'), 'authored');
  } finally { harness.close(); }
});

test('Go actions containing closing braces, comments and quotes survive parsing; damaged fields refuse apply', () => {
  const sources = [
    `<a href='{{printf "%s?source=email" .URL}}'>{{.FirstName}}</a>`,
    '{{printf "}}"}}{{/* }} in a comment */}}{{printf `a}}b`}}',
    'RUPHISH_TEMPLATE_0_END {{.FirstName}}',
  ];
  for (const source of sources) {
    const vault = new TemplateVault(source);
    assert.equal(vault.restore(vault.mask(source)), source);
  }
  assert.throws(() => new TemplateVault('').mask('{{printf "unterminated'), /Unclosed/);
  const vault = new TemplateVault('');
  const masked = vault.mask('{{.FirstName}}');
  assert.throws(() => vault.restore(masked.slice(0, -1)), /partially edited/);
  assert.throws(() => vault.restore(vault.prefix + '999_END'), /unknown template marker/);
});

test('hostile page data is retained through a visual edit without being silently cleaned from the saved document', () => {
  const harness = editor();
  const source = '<!doctype html><html><head><style>p{color:red}</style></head><body><script>window.attack=true</script><p onclick="attack()">Text</p><form action="{{.URL}}"><input type="password" name="password"></form><a href="javascript:attack()">Link</a></body></html>';
  try {
    const session = new VisualSession(harness.value, source);
    harness.value.editor.querySelector('p').append(' edited');
    const saved = session.result();
    for (const content of ['window.attack=true', 'onclick="attack()"', 'name="password"', '{{.URL}}', 'javascript:attack()', 'p{color:red}', 'Text edited']) assert.ok(saved.includes(content), content);
    assert.equal(harness.value.iframe.getAttribute('sandbox'), 'allow-same-origin');
    // jsdom does not enforce browser CSP or sandbox flags. Do not infer isolation from this test.
  } finally { harness.close(); }
});


test('inserting fields leaves the caret after each chip so subsequent input follows it', () => {
  const harness = editor();
  try {
    const session = new VisualSession(harness.value, '<p>Before After</p>');
    const text = harness.value.editor.querySelector('p').firstChild;
    const range = harness.value.ed.createRange();
    range.setStart(text, 7); range.collapse(true);
    harness.value.s.selectRange(range);
    session.insertVariable('FirstName');
    session.insertVariable('LastName');
    harness.value.s.insertHTML('typed');
    const document = new JSDOM(session.result()).window.document;
    assert.equal(document.querySelector('p').textContent.replaceAll('\uFEFF', ''), 'Before {{.FirstName}}{{.LastName}}typedAfter');
  } finally { harness.close(); }
});
