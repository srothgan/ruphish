import test from 'node:test';
import assert from 'node:assert/strict';
import {
  formatDateLabel,
  browserFormatLocale,
  supportsFormatLocale,
  formatLocaleOptions,
  browserTimeZone,
} from '../datetime.mjs';
const value = {
  year: 2026,
  month: 10,
  day: 18,
  hour: 21,
  minute: 53,
  style: 'full',
  include_year: true,
  offset: 'UTC+02:00',
};
const label = (locale, overrides = {}) =>
  formatDateLabel(JSON.stringify({ ...value, ...overrides }), locale);
test('browser locale determines date order and hour cycle independently of zone', () => {
  assert.equal(label('en-US'), '10/18/2026, 09:53 PM (UTC+02:00)');
  assert.equal(label('en-GB'), '18/10/2026, 21:53 (UTC+02:00)');
  assert.equal(label('de-DE'), '18.10.2026, 21:53 (UTC+02:00)');
  assert.equal(label('fr-FR'), '18/10/2026 21:53 (UTC+02:00)');
  assert.equal(label('ja-JP'), '2026/10/18 21:53 (UTC+02:00)');
});
test('chart labels respect supplied date and offset context without a second zone conversion', () => {
  assert.equal(label('en-GB', { style: 'clock', offset: '' }), '21:53');
  assert.equal(label('en-GB', { style: 'date', include_year: false, offset: '' }), '18/10');
  assert.equal(label('en-GB', { style: 'short', include_year: false, offset: '' }), '18/10, 21:53');
  assert.notEqual(
    label('en-GB', { hour: 2, offset: 'UTC+02:00' }),
    label('en-GB', { hour: 2, offset: 'UTC+01:00' }),
  );
  assert.match(label('en-GB', { year: 1 }), /1,/); // Date.UTC must not reinterpret year 1 as 1901.
});
test('format locale can be Chinese independently of Berlin civil time', () => {
  assert.equal(label('zh-CN'), '2026/10/18 21:53 (UTC+02:00)');
  assert.equal(supportsFormatLocale('zh-CN'), true);
  assert.equal(supportsFormatLocale('de-DE'), true);
  assert.throws(() => supportsFormatLocale('invalid_locale'), RangeError);
  assert.equal(browserFormatLocale(), Intl.DateTimeFormat().resolvedOptions().locale);
});
test('detection reads the browser setting without location services', () => {
  assert.equal(browserTimeZone(), Intl.DateTimeFormat().resolvedOptions().timeZone);
});

test('format picker lists supported CLDR regions with translated names and examples', () => {
  const english = JSON.parse(formatLocaleOptions('en', 'en-GB-u-hc-h12', JSON.stringify(value)));
  const german = JSON.parse(formatLocaleOptions('de', 'en-GB-u-hc-h12', JSON.stringify(value)));
  assert.ok(english.length > 100);
  assert.equal(new Set(english.map(([id]) => id)).size, english.length);
  for (const id of ['de-DE', 'en-US', 'en-GB', 'zh-CN', 'en-GB-u-hc-h12']) {
    assert.ok(
      english.some(([key]) => key === id),
      id,
    );
  }
  assert.ok(
    english.every(
      ([id, label]) => supportsFormatLocale(id) && label.includes(id) && label.length > id.length,
    ),
  );
  assert.match(english.find(([id]) => id === 'zh-CN')[1], /Chinese/);
  assert.match(german.find(([id]) => id === 'zh-CN')[1], /Chinesisch/);
  assert.deepEqual(english.map(([id]) => id).sort(), german.map(([id]) => id).sort());
});

// The picker is a promise about the rendered format, not an independent formatter.
test('every advertised format matches the actual date label for the same instant and offset choice', () => {
  for (const offset of ['UTC+02:00', '']) {
    const sample = { ...value, day: 21, hour: 12, minute: 36, offset };
    const json = JSON.stringify(sample);
    const options = JSON.parse(formatLocaleOptions('en', '', json));
    for (const [locale, text] of options) {
      assert.ok(text.endsWith(` — ${formatDateLabel(json, locale)} (${locale})`), locale);
    }
    const german = options.find(([locale]) => locale === 'de-DE')[1];
    assert.ok(german.includes('21.10.2026, 12:36'));
    assert.equal(german.includes('UTC+02:00'), Boolean(offset));
  }
});
