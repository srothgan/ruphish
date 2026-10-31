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

/// Request metadata belongs to the observation, never to a recipient-wide device profile.
#[component]
pub(crate) fn ClientObservation(
    client: Signal<Option<ruphish_domain::client::Info>>,
    kind: Signal<Option<ruphish_domain::events::Kind>>,
) -> impl IntoView {
    use ruphish_domain::{
        client::{Browser, Device, OperatingSystem},
        events::Kind,
    };
    let i18n = ruphish_ui::localization::use_i18n();
    let unknown = move || leptos_i18n::t_string!(i18n, ui.unknown).to_string();
    view! {
        <Show when=move || {
            client.get().is_some()
                || matches!(kind.get(), Some(Kind::LinkClicked | Kind::FormSubmitted))
        }>
            <div class="client-observation">
                <Show
                    when=move || client.get().is_some()
                    fallback=move || {
                        view! {
                            <small>
                                {ruphish_ui::tr!(ui.client_information)}": "
                                {ruphish_ui::tr!(ui.unavailable)}
                            </small>
                        }
                    }
                >
                    <dl>
                        <div>
                            <dt>{ruphish_ui::tr!(ui.client_browser)}</dt>
                            <dd>
                                {move || match client.get().map(|c| c.browser).unwrap_or_default() {
                                    Browser::Chrome => "Chrome".into(),
                                    Browser::Edge => "Edge".into(),
                                    Browser::Firefox => "Firefox".into(),
                                    Browser::Safari => "Safari".into(),
                                    Browser::Opera => "Opera".into(),
                                    Browser::SamsungInternet => "Samsung Internet".into(),
                                    Browser::InternetExplorer => "Internet Explorer".into(),
                                    Browser::Unknown => unknown(),
                                }}
                            </dd>
                        </div>
                        <div>
                            <dt>{ruphish_ui::tr!(ui.client_os)}</dt>
                            <dd>
                                {move || match client.get().map(|c| c.os).unwrap_or_default() {
                                    OperatingSystem::Windows => "Windows".into(),
                                    OperatingSystem::MacOs => "macOS".into(),
                                    OperatingSystem::Linux => "Linux".into(),
                                    OperatingSystem::Android => "Android".into(),
                                    OperatingSystem::Ios => "iOS".into(),
                                    OperatingSystem::ChromeOs => "ChromeOS".into(),
                                    OperatingSystem::Unknown => unknown(),
                                }}
                            </dd>
                        </div>
                        <div>
                            <dt>{ruphish_ui::tr!(ui.client_device)}</dt>
                            <dd>
                                {move || match client.get().map(|c| c.device).unwrap_or_default() {
                                    Device::Desktop => {
                                        leptos_i18n::t_string!(i18n, ui.client_desktop).to_string()
                                    }
                                    Device::Mobile => {
                                        leptos_i18n::t_string!(i18n, ui.client_mobile).to_string()
                                    }
                                    Device::Tablet => {
                                        leptos_i18n::t_string!(i18n, ui.client_tablet).to_string()
                                    }
                                    Device::Unknown => unknown(),
                                }}
                            </dd>
                        </div>
                    </dl>
                </Show>
            </div>
        </Show>
    }
}
