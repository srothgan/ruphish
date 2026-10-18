// DOM integration of the real CodeMirror host and message lifecycle; not browser security proof.
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {JSDOM} from 'jsdom';
import {undo} from '@codemirror/commands';

test('source is mounted initially; failed visual startup can be cancelled; only the active session can apply an undoable change', async () => {
  const dom = new JSDOM('<!doctype html><div id="host"></div>', {url: 'http://localhost/', pretendToBeVisual: true});
  const {window} = dom;
  const names = ['window', 'document', 'navigator', 'MutationObserver', 'requestAnimationFrame', 'cancelAnimationFrame', 'Option', 'Window'];
  const descriptors = new Map(names.map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)]));
  for (const name of names) {
    const value = name.endsWith('AnimationFrame') ? window[name].bind(window) : name === 'window' ? window : window[name];
    Object.defineProperty(globalThis, name, {value, configurable: true, writable: true});
  }
  let editor;
  try {
    const {mountEditor} = await import('../editor.mjs');
    const {EditorView} = await import('@codemirror/view');
    const host = window.document.querySelector('#host');
    const source = '<p>Bonjour José</p><script>window.authoredProbe=true</script>';
    const changes = [];
    const visualStates = [];
    editor = mountEditor(host, source, value => changes.push(value), '["FirstName","URL"]', value => visualStates.push(value));
    const sourcePane = host.querySelector('.source');
    const view = EditorView.findFromDOM(sourcePane.firstElementChild);
    assert.equal(sourcePane.hidden, false);
    assert.equal(view.state.sliceDoc(), source);
    const open = [...host.querySelectorAll('button')].find(button => button.textContent === 'Edit visually');
    const cancel = [...host.querySelectorAll('button')].find(button => button.textContent === 'Cancel visual edits');
    const receive = (frame, data) => window.dispatchEvent(new window.MessageEvent('message', {source: frame.contentWindow, data}));
    const start = () => {
      open.click();
      const frame = host.querySelector('iframe');
      // The trusted UI host must retain its origin, while authored markup must never enter it.
      assert.equal(frame.hasAttribute('sandbox'), false);
      assert.ok(!frame.srcdoc.includes('window.authoredProbe=true'));
      const sent = [];
      frame.contentWindow.postMessage = data => sent.push(data);
      receive(frame, {type: 'booted'});
      return {frame, sent, token: sent[0].token};
    };
    const first = start();
    assert.equal(visualStates.at(-1), true);
    assert.equal(sourcePane.hidden, true);
    assert.equal(cancel.hidden, false);
    receive(first.frame, {type: 'failed', token: first.token});
    assert.match(host.querySelector('.editor-help').textContent, /could not start.*source is unchanged/);
    cancel.click();
    assert.equal(visualStates.at(-1), false);
    assert.equal(sourcePane.hidden, false);
    assert.equal(view.state.sliceDoc(), source);
    assert.equal(host.querySelector('iframe'), null);
    assert.equal(open.disabled, false);
    const second = start();
    receive(second.frame, {type: 'opened', token: second.token});
    receive(second.frame, {type: 'apply', token: first.token, source: 'wrong session'});
    assert.equal(visualStates.at(-1), true);
    assert.equal(view.state.sliceDoc(), source);
    const edited = '<p><strong>Bonjour José</strong> {{.FirstName}}</p>';
    receive(second.frame, {type: 'apply', token: second.token, source: edited});
    assert.equal(visualStates.at(-1), false);
    assert.equal(sourcePane.hidden, false);
    assert.equal(view.state.sliceDoc(), edited);
    assert.equal(changes.at(-1), edited);
    assert.ok(undo(view));
    assert.equal(view.state.sliceDoc(), source);
    start();
    editor.replace('<p>Newly imported template</p>');
    assert.equal(visualStates.at(-1), false);
    assert.equal(host.querySelector('iframe'), null);
    assert.equal(sourcePane.hidden, false);
    assert.equal(view.state.sliceDoc(), '<p>Newly imported template</p>');
    editor.destroy();
    editor = mountEditor(host, source, value => changes.push(value), '["FirstName","URL"]', () => {}, true);
    const readOnlyView = EditorView.findFromDOM(host.querySelector('.source').firstElementChild);
    assert.equal(readOnlyView.contentDOM.getAttribute('contenteditable'), 'false');
    editor.insert('FirstName');
    editor.openVisual();
    assert.equal(host.querySelector('iframe'), null);
    assert.equal(readOnlyView.state.sliceDoc(), source);
    editor.replace('<p>New server content</p>');
    assert.equal(readOnlyView.contentDOM.getAttribute('contenteditable'), 'false');
  } finally {
    editor?.destroy(); window.close();
    for (const [name, descriptor] of descriptors) {
      if (descriptor) Object.defineProperty(globalThis, name, descriptor);
      else delete globalThis[name];
    }
  }
});
