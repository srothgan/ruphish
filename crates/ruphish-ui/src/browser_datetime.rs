//! Small browser boundary: locale formatting and optional local preferences.
use crate::datetime::Label;
use ruphish_domain::datetime::{self, TimeZone};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/web/generated/datetime.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = formatDateLabel)]
    fn format_label(value: &str, locale: Option<&str>) -> Result<String, JsValue>;
    /// Effective browser formatting locale, independently of UI translation.
    #[wasm_bindgen(catch, js_name = browserFormatLocale)]
    pub fn browser_format_locale() -> Result<String, JsValue>;
    /// Whether Intl can honor this explicit date/time formatting locale.
    #[wasm_bindgen(catch, js_name = supportsFormatLocale)]
    pub fn supports_format_locale(locale: &str) -> Result<bool, JsValue>;
    #[wasm_bindgen(catch, js_name = formatLocaleOptions)]
    fn locale_options(locale: &str, selected: &str, sample: &str) -> Result<String, JsValue>;
    /// Human-readable formatting locale for compact browser-default controls.
    #[wasm_bindgen(catch, js_name = formatLocaleName)]
    pub fn format_locale_name(display_locale: &str, locale: &str) -> Result<String, JsValue>;
    #[wasm_bindgen(catch, js_name = browserTimeZone)]
    fn browser_zone() -> Result<String, JsValue>;
}

/// Browser-supported CLDR format choices that also satisfy the server's domain policy.
pub fn format_locale_options(locale: &str, selected: &str, sample: Label) -> Vec<(String, String)> {
    let mut options: Vec<(String, String)> = serde_json::to_string(&sample)
        .ok()
        .and_then(|sample| locale_options(locale, selected, &sample).ok())
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    options.retain(|(value, _)| {
        ruphish_domain::preferences::Preferences {
            format_locale: Some(value.clone()),
            ..Default::default()
        }
        .validate()
        .is_ok()
    });
    options
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
