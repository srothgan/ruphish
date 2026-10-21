//! Reactive account preferences shared by text and date presentation.
use leptos::prelude::*;
use ruphish_contracts::auth::Preferences;
use ruphish_domain::datetime::{self, TimeZone};

/// One authoritative browser copy of the account's persisted overrides.
#[derive(Clone, Copy)]
pub struct DisplayPreferences {
    /// Last successfully loaded or saved account preferences.
    pub value: RwSignal<Preferences>,
    /// Failed refresh feedback; confirmed values remain available.
    pub error: RwSignal<Option<String>>,
    browser_zone: StoredValue<String>,
    revision: StoredValue<u64>,
}

impl DisplayPreferences {
    /// Install before the session bootstrap or any date labels are rendered.
    pub fn provide() -> Self {
        let state = Self {
            value: RwSignal::new(Preferences::default()),
            error: RwSignal::new(None),
            revision: StoredValue::new(0),
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
        self.revision.update_value(|revision| *revision += 1);
        self.error.set(None);
        let locale = value
            .language
            .as_deref()
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(crate::localization::browser_locale);
        if self.value.get_untracked() != value {
            self.value.set(value);
        }
        context.set_locale(locale);
    }

    /// Capture before starting a session read, so a later save takes precedence.
    pub fn read_ticket(self) -> u64 {
        self.revision.get_value()
    }

    /// Apply a refresh only if no newer confirmed preferences have been applied.
    pub fn apply_read(
        self,
        ticket: u64,
        value: Preferences,
        context: leptos_i18n::I18nContext<crate::localization::Locale>,
    ) {
        if ticket == self.revision.get_value() {
            self.apply(value, context);
        }
    }

    /// A failed obsolete read must not overwrite feedback from a newer save.
    pub fn read_failed(self, ticket: u64, error: String) {
        if ticket == self.revision.get_value() {
            self.error.set(Some(error));
        }
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

/// Session revalidation is the single owner of preference refreshes.
#[derive(Clone, Copy)]
pub struct PreferenceRefresh {
    /// Whether the session read is running.
    pub fetching: RwSignal<bool>,
    /// Retry without remounting the account form.
    pub refresh: Callback<()>,
}
