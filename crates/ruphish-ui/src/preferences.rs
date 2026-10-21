//! Account-owned display settings. Only confirmed saves change active presentation.
use crate::{account::Api, feedback::Message, shared::components::*};
use leptos::prelude::*;
use leptos::task::spawn_local_scoped_with_cancellation as spawn_local;
use leptos_i18n::Locale as _;
use ruphish_contracts::auth::Preferences;
use ruphish_domain::datetime;
use ruphish_ui::{
    browser_datetime,
    display_preferences::{DisplayPreferences, PreferenceRefresh},
    localization::{Locale, use_i18n},
    tr, trs,
};

fn optional(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

#[component]
pub(crate) fn DisplayPreferencesForm(api: Api, token: String) -> impl IntoView {
    let i18n = use_i18n();
    let state = expect_context::<DisplayPreferences>();
    let read = expect_context::<PreferenceRefresh>();
    let saved = state.value.get_untracked();
    let baseline = RwSignal::new(saved.clone());
    let language = RwSignal::new(saved.language.unwrap_or_default());
    let format = RwSignal::new(saved.format_locale.unwrap_or_default());
    let zone = RwSignal::new(saved.time_zone.unwrap_or_default());
    let show_offset = RwSignal::new(saved.show_utc_offset);
    let busy = RwSignal::new(false);
    let token = StoredValue::new(token);
    let message = Message::new(String::new());
    let browser_language = ruphish_ui::localization::browser_locale();
    let browser_format =
        StoredValue::new(browser_datetime::browser_format_locale().unwrap_or_else(|_| "en".into()));
    let browser_zone =
        StoredValue::new(datetime::zone_name(&browser_datetime::detected_zone()).to_owned());
    let preview_instant = StoredValue::new(datetime::now());
    let sample = move || {
        let selected_zone = datetime::display_zone(
            optional(zone.get()).as_deref(),
            Some(&browser_zone.get_value()),
        );
        ruphish_ui::datetime::Label::at(preview_instant.get_value(), &selected_zone)
            .with_offset(show_offset.get())
    };
    let browser_format_name = Signal::derive(move || {
        browser_datetime::format_locale_name(
            i18n.get_locale().as_str(),
            &browser_format.get_value(),
        )
        .unwrap_or_else(|_| browser_format.get_value())
    });
    let zone_choices = StoredValue::new(
        datetime::available_zones()
            .into_iter()
            .filter_map(|name| {
                datetime::named_zone(&name).ok().map(|zone| {
                    let offset = preview_instant
                        .get_value()
                        .to_zoned(zone)
                        .strftime("UTC%:z")
                        .to_string();
                    let label = format!("{} ({offset})", name.replace('_', " "));
                    (name, label)
                })
            })
            .collect::<Vec<_>>(),
    );
    let format_options = Memo::new(move |_| {
        let mut choices = vec![(
            String::new(),
            leptos_i18n::t_string!(i18n, preferences.browser_default).to_owned(),
        )];
        choices.extend(browser_datetime::format_locale_options(
            i18n.get_locale().as_str(),
            &format.get(),
            sample(),
        ));
        choices
    });
    let zone_options = Signal::derive(move || {
        let mut choices = vec![(
            String::new(),
            leptos_i18n::t_string!(i18n, preferences.browser_default).to_owned(),
        )];
        choices.extend(zone_choices.get_value());
        choices
    });
    let draft = move || Preferences {
        language: optional(language.get()),
        format_locale: optional(format.get()),
        time_zone: optional(zone.get()),
        show_utc_offset: show_offset.get(),
    };
    // A clean form follows confirmed external changes; unsaved input stays local.
    Effect::new(move |_| {
        let value = state.value.get();
        if !busy.get() && untrack(draft) == baseline.get_untracked() {
            language.set(value.language.clone().unwrap_or_default());
            format.set(value.format_locale.clone().unwrap_or_default());
            zone.set(value.time_zone.clone().unwrap_or_default());
            show_offset.set(value.show_utc_offset);
            baseline.set(value);
        }
    });
    crate::navigation::protect_draft(move || busy.get() || draft() != baseline.get());
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        if busy.get_untracked() {
            return;
        }
        message.clear();
        let input = draft();
        if input.format_locale.as_deref().is_some_and(|locale| {
            !browser_datetime::supports_format_locale(locale).unwrap_or(false)
        }) {
            message.error(leptos_i18n::t_string!(i18n, preferences.invalid_format).to_owned());
            return;
        }
        if input
            .time_zone
            .as_deref()
            .is_some_and(|zone| datetime::named_zone(zone).is_err())
        {
            message.error(leptos_i18n::t_string!(i18n, preferences.invalid_zone).to_owned());
            return;
        }
        busy.set(true);
        spawn_local(async move {
            let result = match serde_json::to_value(&input) {
                Ok(body) => api
                    .resource(
                        "PUT",
                        "/api/user-preferences",
                        &token.get_value(),
                        Some(&body),
                        None,
                    )
                    .await
                    .ok()
                    .and_then(|(value, _)| ruphish_ui::preference_state::decode(value)),
                Err(_) => None,
            };
            busy.set(false);
            if let Some(value) = result {
                language.set(value.language.clone().unwrap_or_default());
                format.set(value.format_locale.clone().unwrap_or_default());
                zone.set(value.time_zone.clone().unwrap_or_default());
                show_offset.set(value.show_utc_offset);
                baseline.set(value.clone());
                state.apply(value, i18n);
                message.status(leptos_i18n::t_string!(i18n, preferences.saved).to_owned());
            } else {
                message.error(leptos_i18n::t_string!(i18n, preferences.save_failed).to_owned());
            }
        });
    };
    let preview = move || {
        let locale = optional(format.get());
        if locale
            .as_deref()
            .is_some_and(|v| !browser_datetime::supports_format_locale(v).unwrap_or(false))
        {
            return leptos_i18n::t_string!(i18n, preferences.invalid_format).to_owned();
        }
        let zone = optional(zone.get());
        if zone
            .as_deref()
            .is_some_and(|v| datetime::named_zone(v).is_err())
        {
            return leptos_i18n::t_string!(i18n, preferences.invalid_zone).to_owned();
        }
        browser_datetime::render_locale(sample(), locale.as_deref())
    };
    view! {
        <Card class="account-panel">
            <h2>{tr!(preferences.heading)}</h2>
            <p>{tr!(preferences.description)}</p>
            <QueryStatus
                fetching=Signal::derive(move || read.fetching.get())
                has_data=Signal::derive(|| true)
                error=Signal::derive(move || state.error.get())
                noun=trs!(preferences.heading)
                refresh=read.refresh
            />
            <form class="account-form" on:submit=submit>
                <div class="preference-field">
                    <Select
                        label=trs!(language.label)
                        value=language
                        disabled=busy
                        on_change=Callback::new(move |value| language.set(value))
                    >
                        <option value="">{tr!(preferences.browser_default)}</option>
                        {Locale::get_all()
                            .iter()
                            .map(|locale| {
                                view! {
                                    <option value=locale.as_str() lang=locale.as_str()>
                                        {leptos_i18n::td_string!(*locale, language.native_name)}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </Select>
                    <div class="preference-reset">
                        <ActionButton
                            style=ButtonStyle::Ghost
                            size=ControlSize::Small
                            disabled=Signal::derive(move || busy.get() || language.get().is_empty())
                            on_click=move |_| language.set(String::new())
                        >
                            {move || {
                                leptos_i18n::t_string!(
                                    i18n, preferences.reset_default, value = leptos_i18n::td_string!(browser_language, language.native_name)
                                )
                            }}
                        </ActionButton>
                    </div>
                </div>
                <div class="preference-field">
                    <SearchSelect
                        id="date-format"
                        label=trs!(preferences.format_label)
                        search_label=trs!(preferences.search_formats)
                        value=format
                        options=format_options
                        disabled=busy
                    />
                    <div class="preference-reset">
                        <ActionButton
                            style=ButtonStyle::Ghost
                            size=ControlSize::Small
                            disabled=Signal::derive(move || busy.get() || format.get().is_empty())
                            on_click=move |_| format.set(String::new())
                        >
                            {move || {
                                leptos_i18n::t_string!(
                                    i18n, preferences.reset_default, value = browser_format_name.get()
                                )
                            }}
                        </ActionButton>
                    </div>
                </div>
                <div class="preference-field">
                    <SearchSelect
                        id="time-zone"
                        label=trs!(preferences.zone_label)
                        search_label=trs!(preferences.search_zones)
                        value=zone
                        options=zone_options
                        disabled=busy
                    />
                    <div class="preference-reset">
                        <ActionButton
                            style=ButtonStyle::Ghost
                            size=ControlSize::Small
                            disabled=Signal::derive(move || busy.get() || zone.get().is_empty())
                            on_click=move |_| zone.set(String::new())
                        >
                            {move || {
                                leptos_i18n::t_string!(
                                    i18n, preferences.reset_default, value = browser_zone.get_value()
                                )
                            }}
                        </ActionButton>
                    </div>
                </div>
                <Checkbox
                    label=trs!(preferences.show_offset)
                    checked=show_offset
                    disabled=busy
                    on_toggle=Callback::new(move |value| show_offset.set(value))
                />
                <p>
                    <strong>{tr!(preferences.preview)}</strong>
                    " "
                    {preview}
                </p>
                {message.view()}
                <div class="account-actions">
                    <ActionButton kind="submit" style=ButtonStyle::Primary disabled=busy>
                        {move || {
                            if busy.get() {
                                leptos_i18n::t_string!(i18n, ui.saving)
                            } else {
                                leptos_i18n::t_string!(i18n, preferences.save)
                            }
                        }}
                    </ActionButton>
                </div>
            </form>
        </Card>
    }
}
