import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseHTML } from 'linkedom';
import { setupNavigation } from '../src/navigation.mjs';

function fixture(matches = true) {
  const { document, window } = parseHTML(
    '<html><body><header><button aria-controls="nav" aria-expanded="false" hidden>Menu</button><nav id="nav"><a href="/docs/"><span>Docs</span></a><a href="https://demo.ruphish.com/">Demo</a></nav></header><main>Content</main></body></html>',
  );
  const header = document.querySelector('header');
  const button = document.querySelector('button');
  const nav = document.querySelector('nav');
  const mobile = new EventTarget();
  mobile.matches = matches;
  assert.equal(nav.hidden, false, 'links are available before JavaScript runs');
  setupNavigation(header, mobile);
  const click = (element) => element.dispatchEvent(new window.Event('click', { bubbles: true }));
  return { document, window, header, button, nav, mobile, click };
}

test('mobile navigation opens only on click and closes on a second click or link selection', () => {
  const { button, nav, click } = fixture();
  assert.equal(button.hidden, false);
  assert.equal(nav.hidden, true);
  click(button);
  assert.equal(nav.hidden, false);
  assert.equal(button.getAttribute('aria-expanded'), 'true');
  click(button);
  assert.equal(nav.hidden, true);
  assert.equal(button.getAttribute('aria-expanded'), 'false');
  click(button);
  click(nav.querySelector('span'));
  assert.equal(nav.hidden, true);
});

test('Escape closes the menu and returns focus to its button', () => {
  const { window, button, nav, click } = fixture();
  let focused = false;
  button.focus = () => {
    focused = true;
  };
  click(button);
  const escape = new window.Event('keydown', { bubbles: true, cancelable: true });
  escape.key = 'Escape';
  nav.querySelector('a').dispatchEvent(escape);
  assert.equal(nav.hidden, true);
  assert.equal(focused, true);
  assert.equal(escape.defaultPrevented, true);
});

test('clicking or moving keyboard focus outside closes the menu', () => {
  const { document, window, header, button, nav, click } = fixture();
  click(button);
  click(document.querySelector('main'));
  assert.equal(nav.hidden, true);
  click(button);
  const internalFocus = new window.Event('focusout', { bubbles: true });
  internalFocus.relatedTarget = nav.querySelector('a');
  header.dispatchEvent(internalFocus);
  assert.equal(nav.hidden, false);
  const externalFocus = new window.Event('focusout', { bubbles: true });
  externalFocus.relatedTarget = document.querySelector('main');
  header.dispatchEvent(externalFocus);
  assert.equal(nav.hidden, true);
});

test('desktop links stay visible and returning to mobile resets the menu', () => {
  const { button, nav, mobile, click } = fixture(false);
  assert.equal(button.hidden, true);
  assert.equal(nav.hidden, false);
  mobile.matches = true;
  mobile.dispatchEvent(new Event('change'));
  assert.equal(nav.hidden, true);
  assert.equal(button.hidden, false);
  click(button);
  mobile.matches = false;
  mobile.dispatchEvent(new Event('change'));
  assert.equal(nav.hidden, false);
  assert.equal(button.hidden, true);
  assert.equal(button.getAttribute('aria-expanded'), 'false');
  mobile.matches = true;
  mobile.dispatchEvent(new Event('change'));
  assert.equal(nav.hidden, true);
});
