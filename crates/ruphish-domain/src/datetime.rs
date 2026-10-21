//! Shared instant parsing, UTC persistence and IANA-zone conversion.
//!
//! Presentation chooses a locale separately; time-zone rules have one owner here.
pub use jiff::{Timestamp, Zoned, tz::TimeZone};

/// Go's absent-date sentinel, retained at legacy API/storage boundaries.
pub const ZERO: &str = "0001-01-01T00:00:00Z";
/// The sentinel as an instant, without fallible initialization.
pub const ZERO_TIMESTAMP: Timestamp = Timestamp::constant(-62_135_596_800, 0);

/// Safe date/time errors without echoing arbitrary input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// An instant needs an explicit offset and a valid calendar date.
    #[error("Invalid timestamp; use an ISO 8601 date/time with a UTC offset")]
    InvalidInstant,
    /// Civil input is deliberately distinct from an instant.
    #[error("Local date/time must not include an offset or time-zone annotation")]
    OffsetInLocal,
    /// Invalid calendar date/time.
    #[error("Invalid local date/time")]
    InvalidLocal,
    /// Unknown or non-IANA time-zone identifier.
    #[error("Unknown time zone")]
    UnknownZone,
    /// Arithmetic or conversion outside the supported range.
    #[error("Date/time is outside the supported range")]
    OutOfRange,
}

/// Parse an RFC3339 instant with nanosecond precision. No implicit local zone.
pub fn parse(value: &str) -> Result<Timestamp, Error> {
    let b = value.as_bytes();
    // Keep the existing RFC3339 wire grammar rather than accepting Jiff's
    // additional Temporal annotations, date-only values or omitted seconds.
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || !matches!(b[10], b'T' | b't')
        || b[13] != b':'
        || b[16] != b':'
        || value.contains(['[', ']'])
    {
        return Err(Error::InvalidInstant);
    }
    value.parse().map_err(|_| Error::InvalidInstant)
}

/// Read legacy SQL timestamps whose date/time separator was a space.
pub fn parse_stored(value: &str) -> Result<Timestamp, Error> {
    parse(&value.replacen(' ', "T", 1))
}

/// Canonical UTC RFC3339 output, retaining fractional precision.
pub fn utc(value: Timestamp) -> String {
    value.to_string()
}

/// Convert integer UTC seconds to canonical output.
pub fn utc_seconds(seconds: i64) -> Result<String, Error> {
    from_seconds(seconds).map(utc)
}

/// Validate integer UTC seconds without truncation or saturation.
pub fn from_seconds(seconds: i64) -> Result<Timestamp, Error> {
    Timestamp::from_second(seconds).map_err(|_| Error::OutOfRange)
}

/// Current UTC instant. Browser binaries must enable Jiff's `js` feature.
pub fn now() -> Timestamp {
    Timestamp::now()
}

/// Strictly increasing modification times for optimistic concurrency.
pub fn next_revision(previous: Option<&str>, current: Timestamp) -> Result<String, Error> {
    let previous = previous.map(parse_stored).transpose()?;
    if let Some(previous) = previous.filter(|previous| *previous >= current) {
        return previous
            .checked_add(jiff::SignedDuration::from_nanos(1))
            .map(utc)
            .map_err(|_| Error::OutOfRange);
    }
    Ok(utc(current))
}

/// Resolve a named IANA zone; numeric offsets are not user zone preferences.
pub fn named_zone(name: &str) -> Result<TimeZone, Error> {
    TimeZone::get(name).map_err(|_| Error::UnknownZone)
}

/// Enumerate selectable identifiers from the same database used for conversion.
pub fn available_zones() -> Vec<String> {
    let mut zones = jiff::tz::db()
        .available()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    zones.push("UTC".to_owned());
    zones.sort_unstable();
    zones.dedup();
    zones
}

/// Choose a valid explicit preference, detected zone, then UTC.
pub fn display_zone(preferred: Option<&str>, detected: Option<&str>) -> TimeZone {
    preferred
        .and_then(|name| named_zone(name).ok())
        .or_else(|| detected.and_then(|name| named_zone(name).ok()))
        .unwrap_or(TimeZone::UTC)
}

/// IANA identifier for labels, with an explicit UTC fallback.
pub fn zone_name(zone: &TimeZone) -> &str {
    zone.iana_name().unwrap_or("UTC")
}

/// Resolve a wall-clock time with iCalendar-compatible DST rules: the first
/// repeated time, or forward by the actual gap (which need not be one hour).
pub fn resolve_local(local: &str, zone: &str) -> Result<Timestamp, Error> {
    if local
        .split_once(['T', 't', ' '])
        .is_none_or(|(_, clock)| clock.contains(['+', '-', 'Z', 'z', '[', ']']))
    {
        return Err(Error::OffsetInLocal);
    }
    let date: jiff::civil::DateTime = local.parse().map_err(|_| Error::InvalidLocal)?;
    named_zone(zone)?
        .to_ambiguous_zoned(date)
        .compatible()
        .map(|date| date.timestamp())
        .map_err(|_| Error::OutOfRange)
}

/// Value for a datetime-local form control in the selected display zone.
pub fn local_input(value: Timestamp, zone: &TimeZone) -> String {
    value
        .to_zoned(zone.clone())
        .strftime("%Y-%m-%dT%H:%M")
        .to_string()
}
