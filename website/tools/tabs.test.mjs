import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseHTML } from 'linkedom';
import { setupTabs } from '../src/tabs.mjs';

function fixture() {
  const { document, window } = parseHTML(
    '<div data-tab-group><div role="tablist"><button role="tab" aria-selected="true" tabindex="0">Docker</button><button role="tab" aria-selected="false" tabindex="-1">Linux/macOS</button><button role="tab" aria-selected="false" tabindex="-1">Windows</button></div><section role="tabpanel">Compose instructions</section><section role="tabpanel" hidden>Shell command</section><section role="tabpanel" hidden>PowerShell command</section></div>',
  );
  const group = document.querySelector('[data-tab-group]');
  const tabs = [...group.querySelectorAll('[role=tab]')];
  const panels = [...group.querySelectorAll('[role=tabpanel]')];
  let focused;
  tabs.forEach((tab, index) => {
    tab.focus = () => {
      focused = index;
    };
  });
  setupTabs(group);
  function key(index, value) {
    const event = new window.Event('keydown', { bubbles: true, cancelable: true });
    event.key = value;
    tabs[index].dispatchEvent(event);
    return event.defaultPrevented;
  }
  return { window, tabs, panels, key, focused: () => focused };
}

test('selecting an installation method exposes only its instructions and updates tab semantics', () => {
  const { window, tabs, panels } = fixture();
  tabs[2].dispatchEvent(new window.Event('click', { bubbles: true }));
  assert.deepEqual(
    panels.map((panel) => panel.hidden),
    [true, true, false],
  );
  assert.deepEqual(
    tabs.map((tab) => tab.getAttribute('aria-selected')),
    ['false', 'false', 'true'],
  );
  assert.deepEqual(
    tabs.map((tab) => tab.getAttribute('tabindex')),
    ['-1', '-1', '0'],
  );
});

test('arrow keys wrap, Home/End select endpoints, and focus follows selection', () => {
  const page = fixture();
  assert.equal(page.key(0, 'ArrowLeft'), true);
  assert.equal(page.focused(), 2);
  assert.equal(page.panels[2].hidden, false);
  assert.equal(page.key(2, 'ArrowRight'), true);
  assert.equal(page.focused(), 0);
  page.key(0, 'End');
  assert.equal(page.focused(), 2);
  page.key(2, 'Home');
  assert.equal(page.focused(), 0);
  assert.equal(page.key(0, 'Tab'), false);
  assert.deepEqual(
    page.panels.map((panel) => panel.hidden),
    [false, true, true],
  );
});
