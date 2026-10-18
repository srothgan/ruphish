import {test} from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {undo, redo} from '@codemirror/commands';
import {CompletionContext} from '@codemirror/autocomplete';
import {createState, templateCompletions} from '../state.mjs';

const fixture = JSON.parse(fs.readFileSync(new URL('../../../../tests/fixtures/compatibility/editor-template.json', import.meta.url)));

function document(source) {
  let state = createState(source);
  return {get state() { return state; }, dispatch(transaction) { state = transaction.state; }};
}

test('Go-exported template edits, undo, redo and reopen retain public fields and attachment bytes', () => {
  const editor = document(fixture.html);
  editor.dispatch(editor.state.update({changes: {from: 0, insert: '<!-- edited -->'}}));
  const reopened = JSON.parse(JSON.stringify({...fixture, html: editor.state.sliceDoc()}));
  assert.equal(reopened.html, '<!-- edited -->' + fixture.html);
  assert.deepEqual(reopened.attachments, fixture.attachments);
  assert.equal(Buffer.from(reopened.attachments[0].content, 'base64').toString('hex'), '000102ff');
  assert.ok(undo(editor));
  assert.deepEqual({...fixture, html: editor.state.sliceDoc()}, fixture);
  assert.ok(redo(editor));
  assert.equal(editor.state.sliceDoc(), reopened.html);
});

test('full documents, Go expressions, hostile markup and mixed newlines survive edit/undo/save', () => {
  const cases = [
    '', '<!doctype html><html><head><style>td{width:50%}</style></head><body><table><tr><td>x</td></tr></table></body></html>',
    '<form action=\'{{printf "%s" .URL}}\'><input name="password" type="password"></form>',
    '<script>alert(1)</script><img src="https://example.test/tracker" onerror="alert(2)"><iframe srcdoc="x"></iframe>',
    '{{if .FirstName}}<p>{{.FirstName}}</p>{{else}}Hello{{end}}',
    '<!--[if mso]><table><![endif]-->© &nbsp; 👩‍💻 Grüße',
    'one\r\ntwo\nthree\rfour\r\n', '<p><broken quote="',
  ];
  for (const source of cases) {
    const editor = document(source);
    assert.equal(editor.state.sliceDoc(), source);
    editor.dispatch(editor.state.update({changes: {from: editor.state.doc.length, insert: '{{.URL}}'}}));
    assert.equal(editor.state.sliceDoc(), source + '{{.URL}}');
    assert.ok(undo(editor));
    assert.equal(JSON.parse(JSON.stringify({html: editor.state.sliceDoc()})).html, source);
    assert.ok(redo(editor));
    assert.equal(editor.state.sliceDoc(), source + '{{.URL}}');
  }
});

test('completion replaces only an unfinished Go expression after Unicode and is undoable', () => {
  const source = '<p>👩‍💻 Grüße {{.Fi';
  const editor = document(source);
  const result = templateCompletions(new CompletionContext(editor.state, editor.state.doc.length, true), ['FirstName', 'URL']);
  const option = result.options.find(item => item.label === '{{.FirstName}}');
  editor.dispatch(editor.state.update({changes: {from: result.from, to: editor.state.doc.length, insert: option.label}}));
  assert.equal(editor.state.sliceDoc(), '<p>👩‍💻 Grüße {{.FirstName}}');
  assert.ok(undo(editor));
  assert.equal(editor.state.sliceDoc(), source);
  for (const closed of ['{{.FirstName}} text', '<p>ordinary HTML</p>', '{{printf "%s" .URL}}']) {
    const state = createState(closed);
    assert.equal(templateCompletions(new CompletionContext(state, state.doc.length, true), ['URL']), null);
  }
});
