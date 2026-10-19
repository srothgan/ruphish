import {test} from 'node:test';
import assert from 'node:assert/strict';
import {JSDOM} from 'jsdom';
import {bindAnchoredPanel} from '../anchored-panel.mjs';

test('state explanations switch without stacking, close from their icon and Escape restores focus', () => {
  const dom = new JSDOM('<button id="a">About Ready</button><div id="pa" hidden><p>Queued</p><button data-panel-close><svg></svg></button></div><button id="b">About Unknown</button><div id="pb" hidden><p>Check the mail server</p><button data-panel-close>Close</button></div><input id="outside">');
  const win = dom.window;
  const [a, pa, b, pb, outside] = ['a', 'pa', 'b', 'pb', 'outside'].map(id => win.document.getElementById(id));
  const removeA = bindAnchoredPanel(a, pa, win);
  const removeB = bindAnchoredPanel(b, pb, win);
  try {
    a.click();
    assert.equal(pa.hidden, false);
    assert.equal(a.getAttribute('aria-expanded'), 'true');
    assert.equal(win.document.activeElement, pa.querySelector('button'));
    pa.querySelector('p').click();
    assert.equal(pa.hidden, false, 'reading the explanation must not dismiss it');
    b.click();
    assert.equal(pa.hidden, true);
    assert.equal(pb.hidden, false);
    win.dispatchEvent(new win.KeyboardEvent('keydown', {key: 'Escape', cancelable: true}));
    assert.equal(pb.hidden, true);
    assert.equal(win.document.activeElement, b);
    a.click();
    pa.querySelector('svg').dispatchEvent(new win.MouseEvent('click', {bubbles: true}));
    assert.equal(pa.hidden, true);
    assert.equal(win.document.activeElement, a);
    a.click();
    outside.focus();
    assert.equal(pa.hidden, true);
    assert.equal(win.document.activeElement, outside);
    a.click();
    outside.dispatchEvent(new win.Event('pointerdown', {bubbles: true}));
    assert.equal(pa.hidden, true);
    removeA();
    removeA();
    a.click();
    assert.equal(pa.hidden, true, 'unmounted controls have no remaining handlers');
  } finally {
    removeA();
    removeB();
    win.close();
  }
});
