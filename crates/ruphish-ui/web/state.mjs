import {EditorError} from './editor-i18n.mjs';
// Shared by the browser view and functional tests: actual CodeMirror document/history.
import {EditorState, Transaction} from '@codemirror/state';
import {history, isolateHistory} from '@codemirror/commands';
import {html} from '@codemirror/lang-html';

export function createState(source, extensions = []) {
  return EditorState.create({doc: source, extensions: [
    // Treat CR as source text. This also retains mixed CRLF/LF and lone CR documents.
    EditorState.lineSeparator.of('\n'), history(), html({autoCloseTags: false}), extensions,
  ]});
}

export function applyVisual(state, original, next) {
  if (state.sliceDoc() !== original) throw new EditorError('source_changed_error');
  if (next === original) return null;
  return state.update({changes: {from: 0, to: state.doc.length, insert: next}, annotations: [Transaction.userEvent.of('input.visual'), isolateHistory.of('full')]});
}

export function templateCompletions(context, variables) {
  const match = context.matchBefore(/\{\{\s*\.?[A-Za-z]*$/);
  if (!match) return null;
  return {
    from: match.from,
    options: variables.map(name => ({label: `{{.${name}}}`, type: 'variable'})),
    validFor: /^\{\{\s*\.?[A-Za-z]*$/,
  };
}
