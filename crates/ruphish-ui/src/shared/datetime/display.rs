//! Browser environment, reactive display preferences and shared date labels.
use crate::shared::datetime::Label;
use crate::shared::datetime::instant as parse_value;
use crate::shared::display_preferences::DisplayPreferences;
use leptos::prelude::*;
use ruphish_domain::datetime::{self, TimeZone};
use serde_json::Value;

pub(crate) fn zone() -> TimeZone {
    use_context::<DisplayPreferences>()
        .map(DisplayPreferences::zone)
        .unwrap_or_else(crate::shared::datetime::browser::detected_zone)
}
fn render(label: Label) -> String {
    if let Some(preferences) = use_context::<DisplayPreferences>() {
        preferences.render(label)
    } else {
        crate::shared::datetime::browser::render(label)
    }
}
pub(crate) fn zone_name() -> String {
    datetime::zone_name(&zone()).to_owned()
}

pub(crate) fn instant(value: &Value) -> String {
    let i18n = crate::localization::use_i18n();

    match parse_value(value) {
        Ok(Some(value)) => render(Label::at(value, &zone())),
        Ok(None) => leptos_i18n::t_string!(i18n, ui.not_set).into(),
        Err(_) => leptos_i18n::t_string!(i18n, ui.time_not_recorded).into(),
    }
}
pub(crate) fn tick(seconds: i64, step: i64, first: i64, last: i64) -> String {
    let i18n = crate::localization::use_i18n();

    Label::tick(seconds, step, first, last, &zone())
        .map(render)
        .unwrap_or_else(|_| leptos_i18n::t_string!(i18n, ui.time_not_recorded).into())
}
pub(crate) fn local_input(seconds: Option<i64>, zone: &TimeZone) -> String {
    seconds
        .and_then(|seconds| datetime::from_seconds(seconds).ok())
        .map(|value| datetime::local_input(value, zone))
        .unwrap_or_default()
}
pub(crate) fn parse_input(
    value: &str,
    original: Option<i64>,
    zone: &TimeZone,
) -> Result<Option<i64>, datetime::Error> {
    crate::shared::datetime::input_seconds(value, original, zone)
}
