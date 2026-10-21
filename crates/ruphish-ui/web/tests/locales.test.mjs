import {test} from 'node:test';
import assert from 'node:assert/strict';
import en from '../../locales/en.json' with {type: 'json'};
import de from '../../locales/de.json' with {type: 'json'};
import {editorText} from '../editor-i18n.mjs';

test('the shipped catalogs cover the same UI text and interpolation arguments', () => {
  function compare(english, german, path = '') {
    assert.deepEqual(Object.keys(german).sort(), Object.keys(english).sort(), path);
    for (const [key, value] of Object.entries(english)) {
      const label = `${path}.${key}`;
      if (typeof value === 'object') { compare(value, german[key], label); continue; }
      assert.equal(typeof german[key], 'string', label);
      assert.ok(german[key].trim().length, label);
      const argumentsOf = text => [...text.matchAll(/\{\{\s*(\w+)\s*\}\}/g)].map(match => match[1]).sort();
      assert.deepEqual(argumentsOf(german[key]), argumentsOf(value), label);
      assert.doesNotMatch(german[key], /\p{L}\?\p{L}|\uFFFD|Ã.|Â.|â€/u, label);
    }
  }
  compare(en, de);
});

test('editor chrome falls back to English for an unsupported locale', () => {
  assert.equal(editorText('de', 'edit_visually'), 'Visuell bearbeiten');
  assert.equal(editorText('unsupported', 'edit_visually'), 'Edit visually');
});
