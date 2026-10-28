//! Only the browser locale/event APIs needed by leptos_i18n in the Node runner.
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
const events = new EventTarget();
export function localeWindow() {
  globalThis.Window ??= class { static [Symbol.hasInstance](value) { return value === globalThis; } };
  globalThis.addEventListener = events.addEventListener.bind(events);
  globalThis.removeEventListener = events.removeEventListener.bind(events);
  Object.defineProperty(globalThis, 'navigator', {configurable: true, value: {languages: ['de-DE'], language: 'de-DE'}});
}
export function focusBrowser() { events.dispatchEvent(new Event("focus")); }
export function changeBrowserLocale() {
  globalThis.navigator.languages = ['fr-FR'];
  globalThis.navigator.language = 'fr-FR';
  events.dispatchEvent(new Event('languagechange'));
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = localeWindow)]
    pub fn install();
    #[wasm_bindgen(js_name = focusBrowser)]
    pub fn focus();
    #[wasm_bindgen(js_name = changeBrowserLocale)]
    pub fn change();
}
