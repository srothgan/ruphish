import {editorText, codeMirrorPhrases} from './editor-i18n.mjs';
import {EditorView, keymap, lineNumbers, drawSelection, highlightActiveLine} from '@codemirror/view';
import {defaultKeymap, historyKeymap} from '@codemirror/commands';
import {Compartment, EditorState} from '@codemirror/state';
import {autocompletion, completionKeymap} from '@codemirror/autocomplete';
import {syntaxHighlighting, defaultHighlightStyle} from '@codemirror/language';
import {createState, templateCompletions, applyVisual} from './state.mjs';
import {visualFrameDocument} from './visual-shell.mjs';

export function mountEditor(parent, source, onChange, variablesJson, onVisualState = () => {}, readOnly = false, locale = 'en', allowForms = false) {
  const labels = new Map();
  const t = key => editorText(locale, key);
  const setText = (node, key) => { labels.set(node, key); node.textContent = t(key); };
  const variables = JSON.parse(variablesJson);
  const modes = document.createElement('div'); modes.className = 'editor-mode';
  const mode = document.createElement('strong'); setText(mode, 'source');
  const variableLabel = document.createElement('label'); const variableText = document.createTextNode(''); variableLabel.append(variableText); setText(variableText, 'recipient_field');
  const picker = document.createElement('select'); picker.append(new Option(t('choose_field'), ''));
  for (const name of variables) picker.append(new Option(name, name));
  variableLabel.append(picker); variableLabel.hidden = variables.length === 0;
  const open = document.createElement('button'); open.type = 'button'; setText(open, 'edit_visually'); open.disabled = readOnly; picker.disabled = readOnly;
  const sourceButton = document.createElement('button'); sourceButton.type = 'button'; setText(sourceButton, 'source'); sourceButton.disabled = true;
  const note = document.createElement('p'); note.className = 'editor-help'; note.setAttribute('role', 'status');
  const sourcePane = document.createElement('div'); sourcePane.className = 'source';
  const container = document.createElement('div');
  modes.append(mode, variableLabel, sourceButton, open); parent.append(modes, note, sourcePane, container);
  note.setAttribute('role', 'status'); setText(note, 'source_help');
  const editable = new Compartment();
  const contentLabel = new Compartment();
  const phrases = new Compartment();
  const extensions = [
    phrases.of(EditorState.phrases.of(codeMirrorPhrases(locale))),
    editable.of(EditorView.editable.of(!readOnly)),
    lineNumbers(), drawSelection(), highlightActiveLine(),
    syntaxHighlighting(defaultHighlightStyle),
    keymap.of([...completionKeymap, ...defaultKeymap, ...historyKeymap]),
    autocompletion({override: [context => templateCompletions(context, variables)]}),
    contentLabel.of(EditorView.contentAttributes.of({'aria-label': t('source')})),
    EditorView.updateListener.of(update => {
      if (update.docChanged) onChange(update.state.sliceDoc());
    }),
  ];
  const view = new EditorView({parent: sourcePane, state: createState(source, extensions)});
  let visual = null;
  const closeVisual = () => {
    if (!visual) return;
    window.removeEventListener('message', visual.listener);
    visual.frame.remove(); visual = null;
    onVisualState(false);
    sourcePane.hidden = false; open.disabled = readOnly; sourceButton.disabled = true; picker.disabled = readOnly;
    note.hidden = false; setText(mode, 'source'); note.setAttribute('role', 'status'); setText(note, 'source_help');
    view.dispatch({effects: editable.reconfigure(EditorView.editable.of(!readOnly))});
    view.requestMeasure();
  };
  const editor = {
    setLocale(value) {
      locale = value;
      for (const [node, key] of labels) node.textContent = t(key);
      picker.options[0].textContent = t('choose_field');
      view.dispatch({effects: [contentLabel.reconfigure(EditorView.contentAttributes.of({'aria-label': t('source')})), phrases.reconfigure(EditorState.phrases.of(codeMirrorPhrases(locale)))]});
      if (visual) {
        visual.frame.title = t('visual_title');
        visual.frame.contentWindow.postMessage({type: 'locale', token: visual.token, locale}, '*');
      }
    },
    replace(source) { closeVisual(); view.setState(createState(source, extensions)); editor.setLocale(locale); },
    insert(name) {
      if (readOnly) return;
      if (visual) {
        if (!visual.ready) visual.pending.push(name);
        else visual.frame.contentWindow.postMessage({type: 'insert', token: visual.token, name}, '*');
        return;
      }
      view.dispatch(view.state.replaceSelection(`{{.${name}}}`)); view.focus();
    },
    openSource() {
      if (!visual) return;
      if (!visual.ready) { closeVisual(); return; }
      visual.frame.contentWindow.postMessage({type: 'snapshot', token: visual.token}, '*');
    },
    openVisual() {
      if (readOnly || visual) return;
      closeVisual(); container.replaceChildren();
      const frame = document.createElement('iframe');
      frame.className = 'visual-frame'; frame.title = t('visual_title');
      // This frame contains trusted application controls, not authored HTML.
      // An opaque origin here also blocks Jodit's access to its child document.
      // The child authored-content frame is separately sandboxed without scripts.
      frame.referrerPolicy = 'no-referrer';
      const token = crypto.getRandomValues(new Uint32Array(4)).join('-');
      const base = view.state.sliceDoc();
      const script = new URL('/visual-frame.js', window.location.origin).href;
      const css = new URL('/visual-frame.css', window.location.origin).href;
      frame.srcdoc = visualFrameDocument(script, css, locale, allowForms);
      const listener = event => {
        if (event.source !== frame.contentWindow || !visual || visual.frame !== frame) return;
        const message = event.data;
        if (message?.type === 'booted' && !visual.booted) {
          visual.booted = true;
          frame.contentWindow.postMessage({type: 'open', token, source: base, variables, locale}, '*');
          return;
        }
        if (message?.token !== token) return;
        if (message.type === 'opened') {
          visual.ready = true; onVisualState(false); picker.disabled = readOnly;
          note.hidden = true; labels.delete(note); note.textContent = '';
          for (const name of visual.pending.splice(0)) frame.contentWindow.postMessage({type: 'insert', token, name}, '*');
          return;
        }
        if (message.type === 'failed') {
          note.hidden = false; note.setAttribute('role', 'alert');
          setText(note, 'start_failed');
          return;
        }
        if (message.type === 'invalid') { onVisualState(true); return; }
        if (!['change', 'snapshot'].includes(message.type) || typeof message.source !== 'string') return;
        if (view.state.sliceDoc() !== visual.source) {
          const status = document.createElement('p'); status.setAttribute('role', 'alert');
          setText(status, 'source_changed');
          container.append(status); return;
        }
        const next = message.source;
        const transaction = applyVisual(view.state, visual.source, next);
        visual.source = next;
        if (transaction) view.dispatch(transaction);
        onVisualState(false);
        if (message.type === 'snapshot') closeVisual();
      };
      visual = {frame, listener, token, booted: false, ready: false, pending: [], source: base};
      onVisualState(true);
      window.addEventListener('message', listener);
      view.dispatch({effects: editable.reconfigure(EditorView.editable.of(false))});
      sourcePane.hidden = true; open.disabled = true; sourceButton.disabled = false; picker.disabled = true;
      setText(mode, 'visual'); setText(note, 'loading');
      container.append(frame);
    },
    destroy() { closeVisual(); view.destroy(); parent.replaceChildren(); },
  };
  open.onclick = () => editor.openVisual();
  sourceButton.onclick = () => editor.openSource();
  picker.onchange = () => { if (variables.includes(picker.value)) editor.insert(picker.value); picker.value = ''; };
  return editor;
}

export function replaceSource(editor, source) { editor.replace(source); }
export function destroyEditor(editor) { editor.destroy(); }

export function setEditorLocale(editor, locale) { editor.setLocale(locale); }
