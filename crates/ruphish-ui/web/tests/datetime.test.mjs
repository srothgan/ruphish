import test from 'node:test';
import assert from 'node:assert/strict';
import { formatDateLabel, loadTimeZone, saveTimeZone, browserTimeZone } from '../datetime.mjs';
const value = { year: 2026, month: 10, day: 18, hour: 21, minute: 53, style: 'full', include_year: true, offset: 'UTC+02:00' };
const label = (locale, overrides = {}) => formatDateLabel(JSON.stringify({ ...value, ...overrides }), locale);
test('browser locale determines date order, month names and hour cycle independently of zone', () => {
    assert.equal(label('en-US'), 'October 18, 2026, 09:53 PM (UTC+02:00)');
    assert.equal(label('en-GB'), '18 October 2026, 21:53 (UTC+02:00)');
    assert.equal(label('de-DE'), '18. Oktober 2026, 21:53 (UTC+02:00)');
    assert.equal(label('fr-FR'), '18 octobre 2026, 21:53 (UTC+02:00)');
    assert.equal(label('ja-JP'), '2026年10月18日, 21:53 (UTC+02:00)');
});
test('chart labels respect supplied date and offset context without a second zone conversion', () => {
    assert.equal(label('en-GB', { style: 'clock', offset: '' }), '21:53');
    assert.equal(label('en-GB', { style: 'date', include_year: false, offset: '' }), '18 Oct');
    assert.equal(label('en-GB', { style: 'short', include_year: false, offset: '' }), '18 Oct, 21:53');
    assert.notEqual(label('en-GB', { hour: 2, offset: 'UTC+02:00' }), label('en-GB', { hour: 2, offset: 'UTC+01:00' }));
    assert.match(label('en-GB', { year: 1 }), /1,/); // Date.UTC must not reinterpret year 1 as 1901.
});
test('optional preferences isolate accounts, clear automatic mode, and expose blocked storage to Rust', () => {
    const original = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
    const values = new Map();
    try {
        Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: {
            getItem: key => values.get(key), setItem: (key, value) => values.set(key, value), removeItem: key => values.delete(key)
        } });
        assert.equal(loadTimeZone('account1'), '');
        saveTimeZone('account1', 'Asia/Tokyo');
        assert.equal(loadTimeZone('account1'), 'Asia/Tokyo');
        assert.equal(loadTimeZone('account2'), '');
        saveTimeZone('account1', '');
        assert.equal(loadTimeZone('account1'), '');
        Object.defineProperty(globalThis, 'localStorage', { configurable: true, get() { throw new Error('blocked'); } });
        assert.throws(() => saveTimeZone('account1', 'UTC'), /blocked/);
        assert.throws(() => loadTimeZone('account1'), /blocked/);
    } finally {
        if (original) Object.defineProperty(globalThis, 'localStorage', original);
        else delete globalThis.localStorage;
    }
});
test('detection reads the browser setting without location services', () => {
    assert.equal(browserTimeZone(), Intl.DateTimeFormat().resolvedOptions().timeZone);
});
