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

test('action disclosure closes after an enabled choice, observes disabling and releases handlers', async () => {
  const dom = new JSDOM('<button id="trigger" disabled>Export</button><div id="panel" data-close-after-action hidden><button disabled>Unavailable</button><button id="csv"><svg></svg>CSV</button></div>');
  const win = dom.window;
  const trigger = win.document.getElementById('trigger');
  const panel = win.document.getElementById('panel');
  const csv = win.document.getElementById('csv');
  const remove = bindAnchoredPanel(trigger, panel, win);
  let exports = 0;
  csv.addEventListener('click', () => exports++);
  try {
    trigger.dispatchEvent(new win.MouseEvent('click', {bubbles: true}));
    assert.equal(panel.hidden, true);
    trigger.disabled = false;
    trigger.click();
    assert.equal(win.document.activeElement, csv);
    panel.querySelector('button').dispatchEvent(new win.MouseEvent('click', {bubbles: true}));
    assert.equal(panel.hidden, false, 'disabled choices cannot dismiss the panel');
    csv.querySelector('svg').dispatchEvent(new win.MouseEvent('click', {bubbles: true}));
    assert.equal(exports, 1);
    assert.equal(panel.hidden, true);
    assert.equal(win.document.activeElement, trigger);
    trigger.click();
    trigger.disabled = true;
    await Promise.resolve();
    assert.equal(panel.hidden, true, 'a busy or unavailable trigger cannot leave active choices behind');
    assert.equal(trigger.getAttribute('aria-expanded'), 'false');
    remove();
    trigger.disabled = false;
    trigger.click();
    assert.equal(panel.hidden, true);
  } finally { remove(); win.close(); }
});

test('searchable selection retains choices while editing and stays within a narrow viewport', () => {
  const dom = new JSDOM('<button id="trigger">Recipients</button><div id="panel" data-panel-width="440" hidden><input type="search"><label><input type="checkbox">Alice</label><button data-panel-close>Done</button></div><button id="outside">Outside</button>');
  const win = dom.window;
  Object.defineProperty(win, 'innerWidth', {value: 320});
  const trigger = win.document.getElementById('trigger');
  const panel = win.document.getElementById('panel');
  const remove = bindAnchoredPanel(trigger, panel, win);
  try {
    trigger.click();
    assert.equal(win.document.activeElement, panel.querySelector('input[type=search]'));
    assert.equal(panel.style.width, '296px');
    panel.querySelector('input[type=checkbox]').click();
    assert.equal(panel.hidden, false);
    assert.equal(panel.querySelector('input[type=checkbox]').checked, true);
    panel.querySelector('button').click();
    assert.equal(panel.hidden, true);
    assert.equal(win.document.activeElement, trigger);
    trigger.click();
    assert.equal(panel.querySelector('input[type=checkbox]').checked, true);
    win.document.getElementById('outside').focus();
    assert.equal(panel.hidden, true);
    assert.equal(win.document.activeElement.id, 'outside');
  } finally { remove(); win.close(); }
});
