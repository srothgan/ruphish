import {test} from 'node:test';
import assert from 'node:assert/strict';
import {JSDOM} from 'jsdom';
import {bindAccountMenu} from '../account-menu.mjs';
import {installNavigationGuard, confirmDiscard} from '../navigation.mjs';

function fixture() {
  const dom = new JSDOM('<button id="trigger" aria-expanded="false">Alice</button><div id="panel" hidden><a href="/settings">My account</a><button>Sign out</button></div><input id="outside">', {url: 'https://admin.example.test/groups/new'});
  const win = dom.window;
  const trigger = win.document.querySelector('#trigger');
  const panel = win.document.querySelector('#panel');
  let rect = {left: 18, top: 710, bottom: 758};
  trigger.getBoundingClientRect = () => rect;
  Object.defineProperty(panel, 'scrollHeight', {value: 180});
  win.innerWidth = 1024;
  win.innerHeight = 800;
  const remove = bindAccountMenu(trigger, panel, win);
  return {win, trigger, panel, rect: value => { rect = value; }, remove, close: () => { remove(); win.close(); }};
}

test('account menu focuses its first action, Escape restores focus, outside interaction dismisses without stealing focus', () => {
  const f = fixture();
  try {
    f.trigger.click();
    assert.equal(f.panel.hidden, false);
    assert.equal(f.trigger.getAttribute('aria-expanded'), 'true');
    assert.equal(f.win.document.activeElement, f.panel.querySelector('a'));
    f.win.dispatchEvent(new f.win.KeyboardEvent('keydown', {key: 'Escape', cancelable: true}));
    assert.equal(f.panel.hidden, true);
    assert.equal(f.win.document.activeElement, f.trigger);
    f.trigger.click();
    const outside = f.win.document.querySelector('#outside');
    outside.focus();
    assert.equal(f.panel.hidden, true);
    assert.equal(f.win.document.activeElement, outside);
    f.trigger.click();
    outside.dispatchEvent(new f.win.Event('pointerdown', {bubbles: true}));
    assert.equal(f.panel.hidden, true);
    assert.equal(f.trigger.getAttribute('aria-expanded'), 'false');
  } finally { f.close(); }
});

test('account menu stays inside desktop, narrow and zoomed viewports and moves below top navigation', () => {
  const f = fixture();
  try {
    f.trigger.click();
    assert.equal(f.panel.style.top, '522px');
    assert.equal(f.panel.style.width, '304px');
    f.win.innerWidth = 280;
    f.win.innerHeight = 360;
    f.rect({left: 240, top: 30, bottom: 78});
    f.win.dispatchEvent(new f.win.Event('resize'));
    assert.equal(f.panel.style.width, '256px');
    assert.equal(f.panel.style.left, '12px');
    assert.equal(f.panel.style.top, '86px');
    const viewport = new f.win.EventTarget();
    Object.assign(viewport, {width: 220, height: 160, offsetLeft: 30, offsetTop: 70});
    f.win.visualViewport = viewport;
    f.rect({left: 240, top: 160, bottom: 208});
    f.win.dispatchEvent(new f.win.Event('resize'));
    const left = parseFloat(f.panel.style.left);
    const top = parseFloat(f.panel.style.top);
    assert.ok(left >= 42 && left + parseFloat(f.panel.style.width) <= 238);
    assert.ok(top >= 82 && top + parseFloat(f.panel.style.maxHeight) <= 218);
  } finally { f.close(); }
});

test('canceling dirty navigation keeps the account menu and draft; accepting reaches the router, cleanup leaves no toggle handlers', () => {
  const f = fixture();
  let accepted = false;
  let navigations = 0;
  f.win.confirm = () => accepted;
  const guard = installNavigationGuard(() => true, f.win);
  f.win.addEventListener('click', event => {
    if (event.target.closest('a') && !event.defaultPrevented) navigations++;
    event.preventDefault();
  });
  try {
    f.trigger.click();
    f.panel.querySelector('a').click();
    assert.equal(navigations, 0);
    assert.equal(f.panel.hidden, false);
    accepted = true;
    f.panel.querySelector('a').click();
    assert.equal(navigations, 1);
    assert.equal(f.panel.hidden, true);
    f.remove();
    f.remove();
    f.trigger.click();
    assert.equal(f.panel.hidden, true);
  } finally { guard(); f.close(); }
});

test('direct sign-out only proceeds without a draft or after accepting discard', () => {
  let prompts = 0;
  let accepted = false;
  const win = {confirm: () => { prompts++; return accepted; }};
  assert.equal(confirmDiscard(false, win), true);
  assert.equal(prompts, 0);
  assert.equal(confirmDiscard(true, win), false);
  accepted = true;
  assert.equal(confirmDiscard(true, win), true);
  assert.equal(prompts, 2);
});
