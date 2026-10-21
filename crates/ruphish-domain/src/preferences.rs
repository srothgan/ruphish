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
        if self
            .language
            .as_deref()
            .is_some_and(|v| !matches!(v, "en" | "de"))
        {
            return Err(Error::Language);
        }
        if let Some(value) = &self.format_locale {
            if value.len() > 128 {
                return Err(Error::FormatLocale);
            }
            let tag = language_tags::LanguageTag::parse(value).map_err(|_| Error::FormatLocale)?;
            tag.validate().map_err(|_| Error::FormatLocale)?;
            // Private-use-only and grandfathered tags are not Intl locale identifiers.
            if tag.primary_language().len() < 2 || tag.extended_language().is_some() {
                return Err(Error::FormatLocale);
            }
        }
        if let Some(value) = &self.time_zone
            && (value.len() > 128 || crate::datetime::named_zone(value).is_err())
        {
            return Err(Error::TimeZone);
        }
        Ok(())
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
