//! Small browser boundary: locale formatting and optional local preferences.
use crate::datetime::Label;
use ruphish_domain::datetime::{self, TimeZone};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/web/datetime.mjs")]
extern "C" {
    #[wasm_bindgen(catch, js_name = formatDateLabel)]
    fn format_label(value: &str, locale: Option<&str>) -> Result<String, JsValue>;
    /// Read a browser-local preference; storage restrictions are recoverable.
    #[wasm_bindgen(catch, js_name = loadTimeZone)]
    pub fn load_zone(key: &str) -> Result<String, JsValue>;
    /// Persist or clear the browser-local preference.
    #[wasm_bindgen(catch, js_name = saveTimeZone)]
    pub fn save_zone(key: &str, value: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = browserTimeZone)]
    fn browser_zone() -> Result<String, JsValue>;
}

/// Browser IANA setting, with UTC fallback even if Intl is unavailable/throws.
pub fn detected_zone() -> TimeZone {
    datetime::display_zone(None, browser_zone().ok().as_deref())
}

/// Render in the browser's locale, after Jiff's conversion.
pub fn render(label: Label) -> String {
    render_locale(label, None)
}

/// Explicit locale for embedded consumers and tests of the actual WASM/Intl path.
pub fn render_locale(label: Label, locale: Option<&str>) -> String {
    serde_json::to_string(&label)
        .ok()
        .and_then(|json| format_label(&json, locale).ok())
        .unwrap_or_else(|| {
            format!(
                "{:04}-{:02}-{:02}, {:02}:{:02} {}",
                label.year, label.month, label.day, label.hour, label.minute, label.offset
            )
        })
}
