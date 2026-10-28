//! Shared presence and activity indicators; unknown evidence is never absence.
use leptos::prelude::*;
use ruphish_ui::trs;

#[component]
pub(crate) fn SubmissionFields(
    username: Signal<Option<bool>>,
    password: Signal<Option<bool>>,
) -> impl IntoView {
    view! {
        <div class="submission-fields">
            <FieldPresence label=trs!(ui.event_username_presence) value=username />
            <FieldPresence label=trs!(ui.event_password_presence) value=password />
        </div>
    }
}

#[component]
fn FieldPresence(label: Signal<String>, value: Signal<Option<bool>>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let state = Signal::derive(move || match value.get() {
        Some(true) => leptos_i18n::t_string!(i18n, ui.event_present).to_string(),
        Some(false) => leptos_i18n::t_string!(i18n, ui.event_absent).to_string(),
        None => leptos_i18n::t_string!(i18n, ui.event_not_recorded).to_string(),
    });
    view! {
        <span
            class="field-presence"
            class:is-present=move || value.get() == Some(true)
            title=move || format!("{}: {}", label.get(), state.get())
        >
            <span class="presence-mark" aria-hidden="true">
                {move || match value.get() {
                    Some(true) => "✓",
                    Some(false) => "×",
                    None => "—",
                }}
            </span>
            <span>{move || label.get()}<small>{move || state.get()}</small></span>
        </span>
    }
}

#[component]
pub(crate) fn ActivityIndicator(recorded: Signal<bool>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let description = Signal::derive(move || {
        if recorded.get() {
            leptos_i18n::t_string!(i18n, ui.activity_recorded).to_string()
        } else {
            leptos_i18n::t_string!(i18n, ui.no_observation).to_string()
        }
    });
    view! {
        <span
            class="activity-indicator"
            class:is-present=move || recorded.get()
            role="img"
            aria-label=move || description.get()
            title=move || description.get()
        >
            <span aria-hidden="true">{move || if recorded.get() { "✓" } else { "—" }}</span>
        </span>
    }
}
