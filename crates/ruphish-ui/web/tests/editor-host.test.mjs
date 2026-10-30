// DOM integration of the real CodeMirror host and message lifecycle; not browser security proof.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { JSDOM } from 'jsdom';
import { undo } from '@codemirror/commands';

test('HTML and visual modes share edits, retain line numbers and reject stale sessions', async () => {
  const dom = new JSDOM('<!doctype html><div id="host"></div>', {
    url: 'https://127.0.0.1:3333/pages/7/edit',
    pretendToBeVisual: true,
  });
  const { window } = dom;
  const names = [
    'window',
    'document',
    'navigator',
    'MutationObserver',
    'requestAnimationFrame',
    'cancelAnimationFrame',
    'Option',
    'Window',
  ];
  const descriptors = new Map(
    names.map((name) => [name, Object.getOwnPropertyDescriptor(globalThis, name)]),
  );
  for (const name of names) {
    const value = name.endsWith('AnimationFrame')
      ? window[name].bind(window)
      : name === 'window'
        ? window
        : window[name];
    Object.defineProperty(globalThis, name, { value, configurable: true, writable: true });
  }
  let editor;
  try {
    const { mountEditor } = await import('../editor.mjs');
    const { EditorView } = await import('@codemirror/view');
    const host = window.document.querySelector('#host');
    const toolbar = window.document.createElement('div');
    const content = window.document.createElement('div');
    const action = window.document.createElement('button');
    action.textContent = 'Check page';
    host.append(toolbar, action, content);
    const source = '<p>Bonjour José</p><script>window.authoredProbe=true</script>';
    const changes = [];
    const visualStates = [];
    editor = mountEditor(
      content,
      toolbar,
      source,
      (value) => changes.push(value),
      '["FirstName","URL"]',
      (value) => visualStates.push(value),
    );
    const sourcePane = host.querySelector('.source');
    const view = EditorView.findFromDOM(sourcePane.firstElementChild);
    assert.equal(sourcePane.hidden, false);
    assert.equal(view.state.sliceDoc(), source);
    const open = [...host.querySelectorAll('button')].find(
      (button) => button.textContent === 'Edit visually',
    );
    const sourceButton = [...host.querySelectorAll('button')].find(
      (button) => button.textContent === 'HTML source',
    );
    assert.ok(host.querySelector('.cm-lineNumbers'));
    const receive = (frame, data) =>
      window.dispatchEvent(
        new window.MessageEvent('message', { source: frame.contentWindow, data }),
      );
    const start = () => {
      open.click();
      const frame = host.querySelector('iframe');
      // The trusted UI host must retain its origin, while authored markup must never enter it.
      assert.equal(frame.hasAttribute('sandbox'), false);
      assert.ok(!frame.srcdoc.includes('window.authoredProbe=true'));
      const frameDocument = new window.DOMParser().parseFromString(frame.srcdoc, 'text/html');
      assert.equal(
        frameDocument.querySelector('script[src]').getAttribute('src'),
        'https://127.0.0.1:3333/visual-frame.js',
      );
      assert.equal(
        frameDocument.querySelector('link[rel="stylesheet"]').getAttribute('href'),
        'https://127.0.0.1:3333/visual-frame.css',
      );
      const sent = [];
      frame.contentWindow.postMessage = (data) => sent.push(data);
      receive(frame, { type: 'booted' });
      return { frame, sent, token: sent[0].token };
    };
    const first = start();
    assert.equal(visualStates.at(-1), true);
    assert.equal(sourcePane.hidden, true);
    assert.equal(sourceButton.disabled, false);
    receive(first.frame, { type: 'failed', token: first.token });
    assert.match(
      host.querySelector('.editor-help').textContent,
      /could not start.*source is unchanged/,
    );
    sourceButton.click();
    assert.equal(visualStates.at(-1), false);
    assert.equal(sourcePane.hidden, false);
    assert.equal(view.state.sliceDoc(), source);
    assert.equal(host.querySelector('iframe'), null);
    assert.equal(open.disabled, false);
    window.history.pushState({}, '', '/templates/8/edit');
    const second = start();
    receive(second.frame, { type: 'opened', token: second.token });
    receive(second.frame, { type: 'change', token: first.token, source: 'wrong session' });
    assert.equal(visualStates.at(-1), false);
    assert.equal(view.state.sliceDoc(), source);
    const edited = '<p><strong>Bonjour José</strong> {{.FirstName}}</p>';
    receive(second.frame, { type: 'change', token: second.token, source: edited });
    assert.equal(sourcePane.hidden, true);
    assert.equal(changes.at(-1), edited);
    assert.equal(host.querySelector('.editor-mode label').hidden, false);
    sourceButton.click();
    assert.equal(second.sent.at(-1).type, 'snapshot');
    receive(second.frame, { type: 'invalid', token: second.token });
    assert.equal(sourcePane.hidden, true);
    assert.equal(host.querySelector('iframe'), second.frame);
    assert.equal(visualStates.at(-1), true);
    receive(second.frame, { type: 'snapshot', token: second.token, source: edited });
    assert.equal(visualStates.at(-1), false);
    assert.equal(sourcePane.hidden, false);
    assert.equal(view.state.sliceDoc(), edited);
    assert.equal(changes.at(-1), edited);
    const changeCount = changes.length;
    editor.setLocale('de');
    assert.equal(open.textContent, 'Visuell bearbeiten');
    assert.equal(view.contentDOM.getAttribute('aria-label'), 'HTML-Quelltext');
    assert.equal(view.state.phrase('Completions'), 'Vervollständigungen');
    view.dispatch({ selection: { anchor: 3 } });
    assert.equal(view.contentDOM.getAttribute('aria-label'), 'HTML-Quelltext');
    assert.equal(view.state.phrase('Completions'), 'Vervollständigungen');
    assert.equal(changes.length, changeCount);
    assert.equal(view.state.sliceDoc(), edited);
    assert.ok(undo(view));
    assert.equal(view.state.sliceDoc(), source);
    start();
    editor.replace('<p>Newly imported template</p>');
    assert.equal(visualStates.at(-1), false);
    assert.equal(host.querySelector('iframe'), null);
    assert.equal(sourcePane.hidden, false);
    assert.equal(view.state.sliceDoc(), '<p>Newly imported template</p>');
    assert.ok(host.querySelector('.cm-lineNumbers'));
    assert.equal(view.contentDOM.getAttribute('aria-label'), 'HTML-Quelltext');
    assert.equal(view.state.phrase('Completions'), 'Vervollständigungen');
    editor.setLocale('en');
    editor.destroy();
    assert.equal(toolbar.childElementCount, 0, 'destroy releases the supplied toolbar controls');
    assert.equal(action.isConnected, true, 'feature-owned actions survive editor cleanup');
    editor = mountEditor(
      content,
      toolbar,
      source,
      (value) => changes.push(value),
      '["FirstName","URL"]',
      () => {},
      true,
    );
    const readOnlyView = EditorView.findFromDOM(host.querySelector('.source').firstElementChild);
    assert.equal(readOnlyView.contentDOM.getAttribute('contenteditable'), 'false');
    editor.insert('FirstName');
    editor.openVisual();
    assert.equal(host.querySelector('iframe'), null);
    assert.equal(readOnlyView.state.sliceDoc(), source);
    editor.replace('<p>New server content</p>');
    assert.equal(readOnlyView.contentDOM.getAttribute('contenteditable'), 'false');
    editor.destroy();
    const explanation = '<p>Literal {{.FirstName}} explanation</p>';
    editor = mountEditor(
      content,
      toolbar,
      explanation,
      (value) => changes.push(value),
      '[]',
      () => {},
      false,
      'en',
      false,
    );
    const learningView = EditorView.findFromDOM(host.querySelector('.source').firstElementChild);
    assert.equal(host.querySelector('.editor-mode label').hidden, true);
    editor.openVisual();
    const learningFrame = host.querySelector('iframe');
    const learningMessages = [];
    learningFrame.contentWindow.postMessage = (data) => learningMessages.push(data);
    receive(learningFrame, { type: 'booted' });
    assert.deepEqual(learningMessages[0].variables, []);
    assert.equal(learningMessages[0].source, explanation);
    const learningToken = learningMessages[0].token;
    receive(learningFrame, { type: 'opened', token: learningToken });
    receive(learningFrame, {
      type: 'change',
      token: learningToken,
      source: '<p>Edited explanation</p>',
    });
    [...host.querySelectorAll('button')]
      .find((button) => button.textContent === 'HTML source')
      .click();
    receive(learningFrame, {
      type: 'snapshot',
      token: learningToken,
      source: '<p>Edited explanation</p>',
    });
    assert.equal(learningView.state.sliceDoc(), '<p>Edited explanation</p>');
    assert.equal(changes.at(-1), '<p>Edited explanation</p>');
    assert.equal(host.querySelector('.editor-mode label').hidden, true);
  } finally {
    editor?.destroy();
    window.close();
    for (const [name, descriptor] of descriptors) {
      if (descriptor) {
        Object.defineProperty(globalThis, name, descriptor);
      } else {
        delete globalThis[name];
      }
    }
  }
});
