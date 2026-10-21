//! Presentation inputs shared by native tests and the browser. Jiff owns all
//! parsing and zone conversion; JavaScript only localizes these civil fields.
use ruphish_domain::datetime::{self, Error, TimeZone, Timestamp};
use serde::Serialize;
use serde_json::Value;

/// Missing legacy dates are different from invalid timestamps.
pub fn instant(value: &Value) -> Result<Option<Timestamp>, Error> {
    let parsed = match value {
        Value::Null => return Ok(None),
        Value::String(value) if value.is_empty() || value == datetime::ZERO => return Ok(None),
        Value::String(value) => datetime::parse_stored(value)?,
        Value::Number(value) => {
            datetime::from_seconds(value.as_i64().ok_or(Error::InvalidInstant)?)?
        }
        _ => return Err(Error::InvalidInstant),
    };
    Ok((parsed != datetime::ZERO_TIMESTAMP).then_some(parsed))
}

/// Parse a local filter edit, preserving the original exact instant when the
/// minute-precision control is unchanged (including the later DST occurrence).
pub fn input_seconds(
    value: &str,
    original: Option<i64>,
    zone: &TimeZone,
) -> Result<Option<i64>, Error> {
    if value.is_empty() {
        return Ok(None);
    }
    if let Some(original) = original.and_then(|seconds| datetime::from_seconds(seconds).ok())
        && datetime::local_input(original, zone) == value
    {
        return Ok(Some(original.as_second()));
    }
    datetime::resolve_local(value, datetime::zone_name(zone)).map(|value| Some(value.as_second()))
}

/// Already converted fields. The browser must format them in synthetic UTC,
/// never convert them through the machine zone a second time.
#[derive(Debug, Serialize)]
pub struct Label {
    /// Calendar year.
    pub year: i16,
    /// Calendar month, one based.
    pub month: i8,
    /// Calendar day.
    pub day: i8,
    /// Local hour.
    pub hour: i8,
    /// Local minute; presentation never rounds into the next minute.
    pub minute: i8,
    /// Full date/time, clock only, date only, or short date/time.
    pub style: &'static str,
    /// Include a year in chart labels spanning years.
    pub include_year: bool,
    /// Offset distinguishes repeated clock times during a DST transition.
    pub offset: String,
}
impl Label {
    /// Full readable date/time at minute precision.
    pub fn at(value: Timestamp, zone: &TimeZone) -> Self {
        let date = value.to_zoned(zone.clone());
        Self {
            year: date.year(),
            month: date.month(),
            day: date.day(),
            hour: date.hour(),
            minute: date.minute(),
            style: "full",
            include_year: true,
            offset: date.strftime("UTC%:z").to_string(),
        }
    }

    /// Adaptive chart labels in the same display zone as the rest of the UI.
    pub fn tick(
        seconds: i64,
        step: i64,
        first: i64,
        last: i64,
        zone: &TimeZone,
    ) -> Result<Self, Error> {
        let start = datetime::from_seconds(first)?.to_zoned(zone.clone());
        let end = datetime::from_seconds(last)?.to_zoned(zone.clone());
        let mut label = Self::at(datetime::from_seconds(seconds)?, zone);
        label.style = if step >= 86_400 {
            "date"
        } else if start.date() == end.date() {
            "clock"
        } else {
            "short"
        };
        label.include_year = start.year() != end.year();
        if step >= 86_400 || start.offset() == end.offset() {
            label.offset.clear();
        }
        Ok(label)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unchanged_filters_keep_the_later_dst_occurrence_and_seconds() {
        let zone = datetime::named_zone("Europe/Berlin").unwrap();
        let original = datetime::parse("2026-10-25T01:30:45Z").unwrap().as_second();
        assert_eq!(
            input_seconds("2026-10-25T02:30", Some(original), &zone).unwrap(),
            Some(original)
        );
        assert_eq!(
            input_seconds("2026-10-25T02:30", None, &zone).unwrap(),
            Some(original - 3645)
        );
        assert_eq!(input_seconds("", Some(original), &zone).unwrap(), None);
        assert!(input_seconds("invalid", Some(original), &zone).is_err());
    }

    #[test]
    fn missing_invalid_and_real_epoch_values_remain_distinct() {
        for value in [Value::Null, json!(""), json!(datetime::ZERO)] {
            assert_eq!(instant(&value).unwrap(), None);
        }
        for value in [
            json!(false),
            json!(1.5),
            json!({}),
            json!("bad"),
            json!("2026-10-18T10:00"),
        ] {
            assert!(instant(&value).is_err());
        }
        assert_eq!(
            datetime::utc(instant(&json!(0)).unwrap().unwrap()),
            "1970-01-01T00:00:00Z"
        );
        let date = instant(&json!("2026-10-18T19:53:59.999999999Z"))
            .unwrap()
            .unwrap();
        let label = Label::at(date, &datetime::named_zone("Asia/Tokyo").unwrap());
        assert_eq!((label.day, label.hour, label.minute), (19, 4, 53));
    }

    #[test]
    fn chart_labels_adapt_to_calendar_boundaries_and_distinguish_repeated_hours() {
        let zone = datetime::named_zone("Europe/Berlin").unwrap();
        let sec = |v| datetime::parse(v).unwrap().as_second();
        for (step, first, last, style, year) in [
            (
                60,
                "2026-10-19T12:00:00Z",
                "2026-10-19T12:05:00Z",
                "clock",
                false,
            ),
            (
                300,
                "2026-10-19T12:00:00Z",
                "2026-10-19T12:30:00Z",
                "clock",
                false,
            ),
            (
                3600,
                "2026-10-19T20:00:00Z",
                "2026-10-20T04:00:00Z",
                "short",
                false,
            ),
            (
                86400,
                "2026-10-19T00:00:00Z",
                "2026-10-26T00:00:00Z",
                "date",
                false,
            ),
            (
                432000,
                "2026-10-01T00:00:00Z",
                "2026-10-31T00:00:00Z",
                "date",
                false,
            ),
            (
                86400,
                "2026-12-25T00:00:00Z",
                "2027-01-05T00:00:00Z",
                "date",
                true,
            ),
        ] {
            let label = Label::tick(sec(first), step, sec(first), sec(last), &zone).unwrap();
            assert_eq!(label.style, style);
            assert_eq!(label.include_year, year);
            assert!(label.offset.is_empty());
        }
        let first = sec("2026-10-25T00:00:00Z");
        let last = sec("2026-10-25T04:00:00Z");
        let a = Label::tick(sec("2026-10-25T00:30:00Z"), 1800, first, last, &zone).unwrap();
        let b = Label::tick(sec("2026-10-25T01:30:00Z"), 1800, first, last, &zone).unwrap();
        assert_eq!((a.hour, a.minute), (2, 30));
        assert_eq!((b.hour, b.minute), (2, 30));
        assert_eq!(a.offset, "UTC+02:00");
        assert_eq!(b.offset, "UTC+01:00");
        assert!(Label::tick(i64::MAX, 60, first, last, &zone).is_err());
    }
}
