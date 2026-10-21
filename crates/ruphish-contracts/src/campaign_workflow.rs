//! Complete preparation and duplication responses, separate from permissive writes.
use crate::{campaign_reads::Policy, nullable};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, Serialize)]
struct Selection<T> {
    groups: Vec<T>,
    template: T,
    page: T,
    smtp: T,
}

#[derive(Deserialize, Serialize)]
struct Reference {
    id: i64,
}

#[derive(Deserialize, Serialize)]
struct ReviewedResource {
    id: i64,
    name: String,
    revision: String,
}

#[derive(Deserialize, Serialize)]
struct RecipientAttributes {
    first_name: String,
    last_name: String,
    position: String,
}

#[derive(Deserialize, Serialize)]
struct Conflict {
    email: String,
    kept: RecipientAttributes,
    discarded: RecipientAttributes,
}

#[derive(Deserialize, Serialize)]
struct Preview {
    subject: String,
    html: String,
    text: String,
    visible_sender: String,
    transport_sender: String,
}

#[derive(Deserialize, Serialize)]
struct Review {
    total: usize,
    duplicate_count: usize,
    launch_date: i64,
    #[serde(deserialize_with = "nullable")]
    send_by_date: Option<i64>,
    selection: Selection<ReviewedResource>,
    policy: Policy,
    mailer_enabled: bool,
    conflict_count: usize,
    conflicts: Vec<Conflict>,
    preview: Preview,
}

#[derive(Deserialize, Serialize)]
struct Duplicate {
    name: String,
    url: String,
    selection: Selection<Reference>,
    #[serde(deserialize_with = "duplicate_policy")]
    policy: Value,
}

fn duplicate_policy<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
    let value = Value::deserialize(deserializer)?;
    // An explicit empty object is the preserved legacy unknown-policy response.
    // Missing, null or partially populated policies are incomplete responses.
    if !value.as_object().is_some_and(serde_json::Map::is_empty) {
        serde_json::from_value::<Policy>(value.clone()).map_err(serde::de::Error::custom)?;
    }
    Ok(value)
}

/// Reject incomplete preparation before it can authorize launch or replace a review.
pub fn validate_review(value: &Value) -> Result<(), serde_json::Error> {
    serde_json::from_value::<Review>(value.clone()).map(|_| ())
}

/// Reject incomplete duplicate input instead of silently initializing blank fields.
pub fn validate_duplicate(value: &Value) -> Result<(), serde_json::Error> {
    serde_json::from_value::<Duplicate>(value.clone()).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn review_requires_recipient_counts_versions_policy_schedule_and_preview() {
        let value = json!({"total":1,"duplicate_count":0,"launch_date":123,"send_by_date":null,
            "selection":{"groups":[{"id":1,"name":"Audience","revision":"v1"}],"template":{"id":2,"name":"Email","revision":"v2"},"page":{"id":3,"name":"Page","revision":"v3"},"smtp":{"id":4,"name":"SMTP","revision":"v4"}},
            "policy":{"capture_credentials":false,"capture_passwords":false,"redirect_url":""},"mailer_enabled":true,"conflict_count":0,"conflicts":[],
            "preview":{"subject":"Subject","html":"<p>Preview</p>","text":"Preview","visible_sender":"Sender <sender@example.test>","transport_sender":"bounce@example.test"}});
        assert!(validate_review(&value).is_ok());
        for field in value.as_object().expect("fixture").keys() {
            let mut missing = value.clone();
            missing.as_object_mut().map(|v| v.remove(field));
            assert!(validate_review(&missing).is_err(), "{field}");
        }
        for pointer in [
            "/selection/template",
            "/selection/groups/0",
            "/policy",
            "/preview",
        ] {
            let mut missing = value.clone();
            *missing.pointer_mut(pointer).expect("fixture") = json!({});
            assert!(validate_review(&missing).is_err(), "{pointer}");
        }
    }

    #[test]
    fn duplication_preserves_explicit_unknown_policy_but_rejects_lost_selections() {
        let value = json!({"name":"Copy","url":"https://example.test","selection":{"groups":[{"id":1}],"template":{"id":2},"page":{"id":0},"smtp":{"id":4}},"policy":{}});
        assert!(validate_duplicate(&value).is_ok());
        for field in ["name", "url", "selection", "policy"] {
            let mut missing = value.clone();
            missing.as_object_mut().map(|v| v.remove(field));
            assert!(validate_duplicate(&missing).is_err());
        }
        for pointer in ["/selection/template", "/selection/groups/0"] {
            let mut missing = value.clone();
            *missing.pointer_mut(pointer).expect("fixture") = json!({});
            assert!(validate_duplicate(&missing).is_err());
        }
        for policy in [Value::Null, json!({"capture_credentials":false})] {
            let mut missing = value.clone();
            missing["policy"] = policy;
            assert!(validate_duplicate(&missing).is_err());
        }
    }
}
