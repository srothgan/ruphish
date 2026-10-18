import {EditorView, keymap, lineNumbers, drawSelection, highlightActiveLine} from '@codemirror/view';
import {defaultKeymap, historyKeymap} from '@codemirror/commands';
import {Compartment} from '@codemirror/state';
import {autocompletion, completionKeymap} from '@codemirror/autocomplete';
import {syntaxHighlighting, defaultHighlightStyle} from '@codemirror/language';
import {createState, templateCompletions, applyVisual} from './state.mjs';
import {visualFrameDocument} from './visual-shell.mjs';

export function mountEditor(parent, source, onChange, variablesJson, onVisualState = () => {}, readOnly = false) {
  const variables = JSON.parse(variablesJson);
  const modes = document.createElement('div'); modes.className = 'editor-mode';
  const mode = document.createElement('strong'); mode.textContent = 'HTML source';
  const variableLabel = document.createElement('label'); variableLabel.textContent = 'Insert recipient field ';
  const picker = document.createElement('select'); picker.append(new Option('Choose a field…', ''));
  for (const name of variables) picker.append(new Option(name, name));
  variableLabel.append(picker);
  const open = document.createElement('button'); open.type = 'button'; open.textContent = 'Edit visually'; open.disabled = readOnly; picker.disabled = readOnly;
  const cancel = document.createElement('button'); cancel.type = 'button'; cancel.className = 'text-danger'; cancel.textContent = 'Cancel visual edits'; cancel.hidden = true;
  const note = document.createElement('p'); note.className = 'editor-help'; note.setAttribute('role', 'status');
  const sourcePane = document.createElement('div'); sourcePane.className = 'source';
  const container = document.createElement('div');
  modes.append(mode, variableLabel, open, cancel); parent.append(modes, note, sourcePane, container);
  const sourceHelp = 'Edit the HTML directly, or choose “Edit visually” to format the rendered message. Recipient fields stay as {{…}} until sending.';
  note.setAttribute('role', 'status'); note.textContent = sourceHelp;
  const editable = new Compartment();
  const extensions = [
    editable.of(EditorView.editable.of(!readOnly)),
    lineNumbers(), drawSelection(), highlightActiveLine(),
    syntaxHighlighting(defaultHighlightStyle),
    keymap.of([...completionKeymap, ...defaultKeymap, ...historyKeymap]),
    autocompletion({override: [context => templateCompletions(context, variables)]}),
    EditorView.contentAttributes.of({'aria-label': 'HTML source'}),
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
    sourcePane.hidden = false; open.disabled = readOnly; cancel.hidden = true; variableLabel.hidden = false;
    mode.textContent = 'HTML source'; note.setAttribute('role', 'status'); note.textContent = sourceHelp;
    view.dispatch({effects: editable.reconfigure(EditorView.editable.of(!readOnly))});
  };
  const editor = {
    replace(source) { closeVisual(); view.setState(createState(source, extensions)); },
    insert(name) {
      if (readOnly) return;
      if (visual) {
        if (!visual.ready) visual.pending.push(name);
        else visual.frame.contentWindow.postMessage({type: 'insert', token: visual.token, name}, '*');
        return;
      }
      view.dispatch(view.state.replaceSelection(`{{.${name}}}`)); view.focus();
    },
    openVisual() {
      if (readOnly) return;
      closeVisual(); container.replaceChildren();
      const frame = document.createElement('iframe');
      frame.className = 'visual-frame'; frame.title = 'Visual HTML editor';
      // This frame contains trusted application controls, not authored HTML.
      // An opaque origin here also blocks Jodit's access to its child document.
      // The child authored-content frame is separately sandboxed without scripts.
      frame.referrerPolicy = 'no-referrer';
      const token = crypto.getRandomValues(new Uint32Array(4)).join('-');
      const base = view.state.sliceDoc();
      const script = new URL('visual-frame.js', document.baseURI).href;
      const css = new URL('visual-frame.css', document.baseURI).href;
      frame.srcdoc = visualFrameDocument(script, css);
      const listener = event => {
        if (event.source !== frame.contentWindow || !visual || visual.frame !== frame) return;
        const message = event.data;
        if (message?.type === 'booted' && !visual.booted) {
          visual.booted = true;
          frame.contentWindow.postMessage({type: 'open', token, source: base, variables}, '*');
          return;
        }
        if (message?.token !== token) return;
        if (message.type === 'opened') {
          visual.ready = true;
          note.textContent = 'Select text and use the formatting toolbar below. Apply visual changes to return to source, or cancel to discard them.';
          for (const name of visual.pending.splice(0)) frame.contentWindow.postMessage({type: 'insert', token, name}, '*');
          return;
        }
        if (message.type === 'failed') {
          note.setAttribute('role', 'alert');
          note.textContent = 'Visual editing could not start. Your HTML source is unchanged. Choose “Cancel visual edits” to return.';
          return;
        }
        if (message.type !== 'apply' || typeof message.source !== 'string') return;
        if (view.state.sliceDoc() !== base) {
          const status = document.createElement('p'); status.setAttribute('role', 'alert');
          status.textContent = 'Source changed while visual editing was open. Close and reopen visual editing to use the current document.';
          container.append(status); return;
        }
        const next = message.source;
        closeVisual();
        const transaction = applyVisual(view.state, base, next);
        if (transaction) view.dispatch(transaction);
      };
      visual = {frame, listener, token, booted: false, ready: false, pending: []};
      onVisualState(true);
      window.addEventListener('message', listener);
      view.dispatch({effects: editable.reconfigure(EditorView.editable.of(false))});
      sourcePane.hidden = true; open.disabled = true; cancel.hidden = false; variableLabel.hidden = true;
      mode.textContent = 'Visual editor'; note.textContent = 'Loading visual editing. You can cancel at any time; HTML source is unchanged until you apply.';
      container.append(frame);
    },
    destroy() { closeVisual(); view.destroy(); parent.replaceChildren(); },
  };
  open.onclick = () => editor.openVisual();
  cancel.onclick = () => closeVisual();
  picker.onchange = () => { if (variables.includes(picker.value)) editor.insert(picker.value); picker.value = ''; };
  return editor;
}

export function replaceSource(editor, source) { editor.replace(source); }
export function destroyEditor(editor) { editor.destroy(); }
