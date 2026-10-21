//! Validate complete preference reads without changing the permissive write contract.
use ruphish_contracts::auth::Preferences;
use serde_json::Value;

/// Null is a confirmed browser default; an absent field is an incomplete read.
pub fn decode(value: Value) -> Option<Preferences> {
    for field in ["language", "format_locale", "time_zone", "show_utc_offset"] {
        value.get(field)?;
    }
    serde_json::from_value(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn incomplete_reads_cannot_reset_preferences_to_defaults() {
        let valid = json!({"language": null, "format_locale": null, "time_zone": null, "show_utc_offset": false});
        assert!(!decode(valid.clone()).unwrap().show_utc_offset);
        for field in ["language", "format_locale", "time_zone", "show_utc_offset"] {
            let mut incomplete = valid.clone();
            incomplete.as_object_mut().unwrap().remove(field);
            assert!(decode(incomplete).is_none(), "{field}");
        }
        assert!(decode(json!({"language": "en", "format_locale": null, "time_zone": null, "show_utc_offset": "false"})).is_none());
    }
}
