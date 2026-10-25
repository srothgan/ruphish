//! Account-owned display overrides. Absence means use the current browser default.

/// Presentation settings never change stored UTC instants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    /// UI translation locale, independent of date formatting.
    pub language: Option<String>,
    /// BCP-47 locale used by the browser's date formatter.
    pub format_locale: Option<String>,
    /// IANA zone used to convert instants to and from local civil time.
    pub time_zone: Option<String>,
    /// Whether date labels include their UTC offset.
    pub show_utc_offset: bool,
}

/// Invalid preference input, without reflecting arbitrary submitted text.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// UI translations must exist for the requested language.
    #[error("Unsupported interface language")]
    Language,
    /// Formatting requires a well-formed, registered language tag.
    #[error("Invalid date/time format locale")]
    FormatLocale,
    /// An explicit zone must be available in the application's time-zone database.
    #[error("Invalid time zone")]
    TimeZone,
}

impl Preferences {
    /// Validate overrides at the domain boundary; null alone selects browser defaults.
    pub fn validate(&self) -> Result<(), Error> {
        self.validation().finish().map_err(|report| {
            match report.issues().first().map(|i| i.field.as_str()) {
                Some("language") => Error::Language,
                Some("format_locale") => Error::FormatLocale,
                _ => Error::TimeZone,
            }
        })
    }
    /// Domain grammar, separate from browser-specific formatting support.
    pub fn validation(&self) -> crate::validation::Report {
        use crate::validation::{Code, Report};
        let mut report = Report::default();
        if self
            .language
            .as_deref()
            .is_some_and(|v| !matches!(v, "en" | "de"))
        {
            report.reject("language", Code::Invalid, "Unsupported interface language");
        }
        if let Some(value) = &self.format_locale {
            let valid = value.len() <= 128
                && language_tags::LanguageTag::parse(value).is_ok_and(|tag| {
                    tag.validate().is_ok()
                        && tag.primary_language().len() >= 2
                        && tag.extended_language().is_none()
                });
            if !valid {
                report.reject(
                    "format_locale",
                    Code::Invalid,
                    "Invalid date/time format locale",
                );
            }
        }
        if let Some(value) = &self.time_zone
            && (value.len() > 128 || crate::datetime::named_zone(value).is_err())
        {
            report.reject("time_zone", Code::Invalid, "Invalid time zone");
        }
        report
    }
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: None,
            format_locale: None,
            time_zone: None,
            show_utc_offset: true,
        }
    }
}
