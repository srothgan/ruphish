//! Legacy event details are untrusted; only typed observations cross the adapter.
use serde_json::{Value, json};

/// Rebuild permitted event metadata instead of forwarding a legacy payload map.
pub(super) fn details(source: &str) -> String {
    let Ok(Value::Object(value)) = serde_json::from_str(source) else {
        return String::new();
    };
    let mut safe = serde_json::Map::new();
    for key in ["username_present", "password_present"] {
        if let Some(value) = value.get(key).and_then(Value::as_bool) {
            safe.insert(key.into(), json!(value));
        }
    }
    if let Some(code) = value.get("code").and_then(Value::as_str).filter(|code| {
        matches!(
            *code,
            "delivery_outcome_unknown"
                | "smtp_permanent_rejection"
                | "smtp_profile_blocked"
                | "smtp_temporary_rejection"
                | "delivery_retry_exhausted"
        )
    }) {
        safe.insert("code".into(), json!(code));
    }
    if let Some(action) = value
        .get("action")
        .and_then(Value::as_str)
        .filter(|action| matches!(*action, "retry" | "resend_unknown" | "confirm_accepted"))
    {
        safe.insert("action".into(), json!(action));
        if let Some(actor) = value.get("actor").and_then(Value::as_u64) {
            safe.insert("actor".into(), json!(actor));
        }
    }
    if safe.is_empty() {
        String::new()
    } else {
        Value::Object(safe).to_string()
    }
}
