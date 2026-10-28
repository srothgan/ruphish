import test from 'node:test';
import assert from 'node:assert/strict';
import { retainQueryHeight } from '../query-layout.mjs';

test('selection loading preserves the last visible height without capturing hidden or zero-sized content', () => {
    const original = globalThis.ResizeObserver;
    let resized, observed, disconnected = false, height = 420;
    globalThis.ResizeObserver = class {
        constructor(callback) { resized = callback; }
        observe(element) { observed = element; }
        disconnect() { disconnected = true; }
    };
    try {
        const styles = new Map();
        const region = { dataset: { ready: 'false' }, style: { setProperty: (key, value) => styles.set(key, value) } };
        const content = { hidden: true, getBoundingClientRect: () => ({ height }) };
        const release = retainQueryHeight(region, content);
        assert.equal(observed, content);
        resized();
        assert.equal(styles.size, 0, 'initial loading has no fabricated previous size');
        region.dataset.ready = 'true'; content.hidden = false;
        resized();
        assert.equal(styles.get('--query-content-height'), '420px');
        region.dataset.ready = 'false'; content.hidden = true; height = 0;
        resized();
        assert.equal(styles.get('--query-content-height'), '420px');
        region.dataset.ready = 'true';
        resized();
        assert.equal(styles.get('--query-content-height'), '420px', 'hidden ancestors cannot overwrite the reservation');
        content.hidden = false; height = 180;
        resized();
        assert.equal(styles.get('--query-content-height'), '180px', 'a successful empty state can have a different height');
        release();
        assert.equal(disconnected, true);
    } finally {
        globalThis.ResizeObserver = original;
    }
});
