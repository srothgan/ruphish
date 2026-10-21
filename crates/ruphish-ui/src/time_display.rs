//! Browser environment, reactive display preferences and shared date labels.
use crate::{feedback::Message, shared::components::*};
use leptos::prelude::*;
use ruphish_domain::datetime::{self, TimeZone};
use ruphish_ui::datetime::{Label, instant as parse_value};
use serde_json::Value;

use ruphish_ui::browser_datetime::{detected_zone, load_zone, render, save_zone};

#[derive(Clone, Copy)]
struct Preference {
    value: RwSignal<String>,
    detected: StoredValue<String>,
    key: StoredValue<String>,
}

pub(super) fn provide(user: i64) {
    let detected = datetime::zone_name(&detected_zone()).to_owned();
    let key = format!("ruphish.time-zone.{user}");
    let value = load_zone(&key)
        .ok()
        .filter(|value| datetime::named_zone(value).is_ok())
        .unwrap_or_default();
    provide_context(Preference {
        value: RwSignal::new(value),
        detected: StoredValue::new(detected),
        key: StoredValue::new(key),
    });
}

fn zone() -> TimeZone {
    if let Some(preference) = use_context::<Preference>() {
        datetime::display_zone(
            Some(&preference.value.get()),
            Some(&preference.detected.get_value()),
        )
    } else {
        detected_zone()
    }
}
pub(super) fn zone_name() -> String {
    datetime::zone_name(&zone()).to_owned()
}

pub(super) fn instant(value: &Value) -> String {
    match parse_value(value) {
        Ok(Some(value)) => render(Label::at(value, &zone())),
        Ok(None) => "Not set".into(),
        Err(_) => "Time not recorded".into(),
    }
}
pub(super) fn tick(seconds: i64, step: i64, first: i64, last: i64) -> String {
    Label::tick(seconds, step, first, last, &zone())
        .map(render)
        .unwrap_or_else(|_| "Time not recorded".into())
}
pub(super) fn local_input(seconds: Option<i64>) -> String {
    seconds
        .and_then(|seconds| datetime::from_seconds(seconds).ok())
        .map(|value| datetime::local_input(value, &zone()))
        .unwrap_or_default()
}
pub(super) fn parse_input(
    value: &str,
    original: Option<i64>,
) -> Result<Option<i64>, datetime::Error> {
    ruphish_ui::datetime::input_seconds(value, original, &zone())
}

#[component]
pub(super) fn TimeZonePreference() -> impl IntoView {
    let preference = use_context::<Preference>();
    let draft = RwSignal::new(
        preference
            .map(|p| p.value.get_untracked())
            .unwrap_or_default(),
    );
    let message = Message::new(String::new());
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        let Some(preference) = preference else {
            return;
        };
        let value = draft.get_untracked().trim().to_owned();
        if !value.is_empty() && datetime::named_zone(&value).is_err() {
            message.error("Enter a valid IANA time zone, such as America/New_York, or leave it empty for automatic detection.".into());
            return;
        }
        if save_zone(&preference.key.get_value(), &value).is_err() {
            message.error(
                "This browser could not save the preference. Allow local storage and try again."
                    .into(),
            );
            return;
        }
        preference.value.set(value.clone());
        draft.set(value);
        message.status("Time zone saved for your account in this browser.".into());
    };
    view! { <Card class="account-panel"><h2>"Date and time"</h2>
        <p>{move || format!("Times are displayed in {}. Leave the override empty to use your browser’s time zone. This preference applies only to your account in this browser.", zone_name())}</p>
        <form on:submit=submit><Field label="Time zone override (optional)"><TextInput value=Signal::derive(move || draft.get()) placeholder="Automatic" on_input=move |event|draft.set(event_target_value(&event))/></Field>
        {message.view()}<div class="account-actions"><ActionButton kind="submit" style=ButtonStyle::Primary>"Save time zone"</ActionButton></div></form>
    </Card> }
}
