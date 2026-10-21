//! Reactive account preferences shared by text and date presentation.
use leptos::prelude::*;
use ruphish_contracts::auth::Preferences;
use ruphish_domain::datetime::{self, TimeZone};

/// One authoritative browser copy of the account's persisted overrides.
#[derive(Clone, Copy)]
pub struct DisplayPreferences {
    /// Last successfully loaded or saved account preferences.
    pub value: RwSignal<Preferences>,
    browser_zone: StoredValue<String>,
}

impl DisplayPreferences {
    /// Install before the session bootstrap or any date labels are rendered.
    pub fn provide() -> Self {
        let state = Self {
            value: RwSignal::new(Preferences::default()),
            browser_zone: StoredValue::new(
                datetime::zone_name(&crate::browser_datetime::detected_zone()).to_owned(),
            ),
        };
        provide_context(state);
        state
    }

    /// Apply confirmed server state without changing stored instants or remounting views.
    pub fn apply(
        self,
        value: Preferences,
        context: leptos_i18n::I18nContext<crate::localization::Locale>,
    ) {
        let locale = value
            .language
            .as_deref()
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(crate::localization::browser_locale);
        self.value.set(value);
        context.set_locale(locale);
    }

    /// Resolve the display/input zone independently of formatting and text language.
    pub fn zone(self) -> TimeZone {
        datetime::display_zone(
            self.value.get().time_zone.as_deref(),
            Some(&self.browser_zone.get_value()),
        )
    }

    /// Format already-zoned civil fields using only the date-format preference.
    pub fn render(self, label: crate::datetime::Label) -> String {
        let preferences = self.value.get();
        crate::browser_datetime::render_locale(
            label.with_offset(preferences.show_utc_offset),
            preferences.format_locale.as_deref(),
        )
    }
}
