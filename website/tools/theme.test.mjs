import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { parseHTML } from 'linkedom';

const script = await readFile(new URL('../src/theme.js', import.meta.url), 'utf8');
const controls =
  '<div data-theme-control hidden><select data-theme-select aria-label="Color theme"><option value="light">Light</option><option value="dark">Dark</option><option value="auto" selected>System</option></select></div>';

function fixture({
  saved = new Map(),
  dark = false,
  blocked = false,
  initialControls = true,
} = {}) {
  const { document, window } = parseHTML(
    `<html data-theme="dark"><head></head><body>${initialControls ? controls.repeat(2) : ''}</body></html>`,
  );
  const system = new EventTarget();
  system.matches = dark;
  const browser = new EventTarget();
  browser.matchMedia = () => system;
  Object.defineProperty(browser, 'localStorage', {
    get() {
      if (blocked) {
        throw new Error('Storage is unavailable');
      }
      return {
        getItem: (key) => saved.get(key) ?? null,
        setItem: (key, value) => saved.set(key, value),
      };
    },
  });

  function pickers() {
    return [...document.querySelectorAll('select')];
  }
  function supportSelectValue() {
    // Linkedom implements the select value getter but lacks the browser's setter.
    for (const picker of pickers()) {
      Object.defineProperty(picker, 'value', {
        configurable: true,
        get: () => picker.querySelector('option[selected]')?.value,
        set(value) {
          for (const option of picker.options) {
            if (option.value === value) {
              option.setAttribute('selected', '');
            } else {
              option.removeAttribute('selected');
            }
          }
        },
      });
    }
  }
  supportSelectValue();
  runInNewContext(script, {
    window: browser,
    document,
    HTMLSelectElement: window.HTMLSelectElement,
  });
  function choose(value) {
    pickers()[0].value = value;
    pickers()[0].dispatchEvent(new window.Event('change', { bubbles: true }));
  }
  function systemChange(value) {
    system.matches = value;
    system.dispatchEvent(new Event('change'));
  }
  function storageChange(value, key = 'starlight-theme') {
    const event = new Event('storage');
    Object.assign(event, { key, newValue: value });
    browser.dispatchEvent(event);
  }
  function mountControls() {
    document.body.innerHTML = controls;
    supportSelectValue();
    document.dispatchEvent(new window.Event('DOMContentLoaded'));
  }
  return { document, saved, pickers, choose, systemChange, storageChange, mountControls };
}

test('saved explicit preference wins over the system before controls mount', () => {
  const page = fixture({
    saved: new Map([['starlight-theme', 'light']]),
    dark: true,
    initialControls: false,
  });
  assert.equal(page.document.documentElement.dataset.theme, 'light');
  page.mountControls();
  assert.equal(page.pickers()[0].value, 'light');
  assert.equal(page.pickers()[0].closest('[data-theme-control]').hidden, false);
  assert.equal(page.pickers()[0].closest('[data-theme-control]').dataset.preference, 'light');
});

test('saved choice survives a fresh page and synchronizes multiple selectors', () => {
  const publicPage = fixture();
  publicPage.choose('dark');
  assert.equal(publicPage.document.documentElement.dataset.theme, 'dark');
  assert.equal(publicPage.saved.get('starlight-theme'), 'dark');
  assert.deepEqual(
    publicPage.pickers().map((picker) => picker.value),
    ['dark', 'dark'],
  );
  const docsPage = fixture({ saved: publicPage.saved });
  assert.equal(docsPage.document.documentElement.dataset.theme, 'dark');
  docsPage.choose('light');
  const aboutPage = fixture({ saved: docsPage.saved, dark: true });
  assert.equal(aboutPage.document.documentElement.dataset.theme, 'light');
});

test('System follows operating system changes while explicit themes stay fixed', () => {
  const page = fixture();
  assert.equal(page.document.documentElement.dataset.theme, 'light');
  page.systemChange(true);
  assert.equal(page.document.documentElement.dataset.theme, 'dark');
  page.choose('light');
  page.systemChange(false);
  page.systemChange(true);
  assert.equal(page.document.documentElement.dataset.theme, 'light');
  page.choose('auto');
  assert.equal(page.document.documentElement.dataset.theme, 'dark');
  assert.equal(page.saved.get('starlight-theme'), '');
});

test('blocked storage permits in-memory choices without crashing initialization', () => {
  const page = fixture({ blocked: true, dark: true });
  assert.equal(page.document.documentElement.dataset.theme, 'dark');
  page.choose('light');
  page.systemChange(true);
  assert.equal(page.document.documentElement.dataset.theme, 'light');
});

test('another tab can change or clear the saved preference', () => {
  const page = fixture({ dark: true });
  page.storageChange('light');
  assert.equal(page.document.documentElement.dataset.theme, 'light');
  assert.deepEqual(
    page.pickers().map((picker) => picker.value),
    ['light', 'light'],
  );
  page.storageChange('dark', 'unrelated-preference');
  assert.equal(page.document.documentElement.dataset.theme, 'light');
  page.storageChange(null, null);
  assert.equal(page.document.documentElement.dataset.theme, 'dark');
  assert.equal(page.pickers()[0].value, 'auto');
});

test('unrecognized stored themes fall back to System', () => {
  const page = fixture({ saved: new Map([['starlight-theme', 'invalid']]), dark: true });
  assert.equal(page.document.documentElement.dataset.theme, 'dark');
  assert.equal(page.pickers()[0].value, 'auto');
});

test('shared palettes keep normal text, links, badges, and actions readable', async () => {
  const css = await readFile(new URL('../src/styles/theme.css', import.meta.url), 'utf8');
  const rules = [...css.matchAll(/:root(?:\[data-theme='(dark)'\])?\s*\{([^}]+)\}/g)];
  assert.equal(rules.length, 2);
  let palette = {};
  function luminance(hex) {
    return hex
      .match(/\w\w/g)
      .map((part) => Number.parseInt(part, 16) / 255)
      .map((value) => (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4))
      .reduce((sum, value, index) => sum + value * [0.2126, 0.7152, 0.0722][index], 0);
  }
  for (const [, mode, declarations] of rules) {
    palette = {
      ...palette,
      ...Object.fromEntries(
        [...declarations.matchAll(/--([\w-]+):\s*#([\da-f]{6});/g)].map(([, name, hex]) => [
          name,
          hex,
        ]),
      ),
    };
    const pairs = [
      ['ink', 'canvas'],
      ['ink', 'paper'],
      ['muted', 'canvas'],
      ['muted', 'paper'],
      ['muted', 'tint'],
      ['link', 'canvas'],
      ['link', 'paper'],
      ['link', 'tint'],
      ['on-action', 'action'],
      ['on-action', 'action-hover'],
      ['positive-text', 'tint'],
      ['warning-text', 'warning-bg'],
      ['info-text', 'paper'],
      ['danger', 'paper'],
      ['nav-text', 'nav-bg'],
      ['nav-text', 'nav-hover'],
    ];
    for (const [foreground, background] of pairs) {
      const values = [luminance(palette[foreground]), luminance(palette[background])];
      const contrast = (Math.max(...values) + 0.05) / (Math.min(...values) + 0.05);
      assert.ok(
        contrast >= 4.5,
        `${mode ?? 'light'} ${foreground} on ${background}: ${contrast.toFixed(2)}:1`,
      );
    }
  }
});
