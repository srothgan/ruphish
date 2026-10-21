import {test} from 'node:test';
import assert from 'node:assert/strict';
import {browserLanguages} from '../language.mjs';

test('browser language order and regional tags reach the Rust resolver unchanged', () => {
  assert.deepEqual(browserLanguages({navigator: {languages: ['fr-FR', 'de-DE', 'en-US']}}), ['fr-FR', 'de-DE', 'en-US']);
  assert.deepEqual(browserLanguages({navigator: {languages: [], language: 'de-AT'}}), ['de-AT']);
  assert.deepEqual(browserLanguages({}), []);
});
