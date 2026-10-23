//! Rebuild metadata through the closed observation representation.
use ruphish_domain::events::Metadata;
use serde_json::Value;

/// Unknown keys, malformed types and arbitrary payloads never cross this boundary.
pub(crate) fn details(source: &str) -> String {
    let Ok(Value::Object(value)) = serde_json::from_str(source) else {
        return String::new();
    };
    let action = value
        .get("action")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok());
    let actor = action
        .as_ref()
        .and_then(|_| value.get("actor").and_then(Value::as_u64));
    let safe = Metadata {
        username_present: value.get("username_present").and_then(Value::as_bool),
        password_present: value.get("password_present").and_then(Value::as_bool),
        code: value
            .get("code")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok()),
        action,
        actor,
    };
    serde_json::to_string(&safe)
        .ok()
        .filter(|value| value != "{}")
        .unwrap_or_default()
}
