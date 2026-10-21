//! Exercise the production request builder against the JavaScript Fetch implementation.
#![cfg(target_arch = "wasm32")]
#![allow(clippy::unwrap_used)]

use ruphish_ui::browser_http::resource_request;
use serde_json::json;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn timestamps_use_jiff_zones_and_browser_locales_without_double_conversion() {
    use ruphish_domain::datetime::{named_zone, parse};
    use ruphish_ui::{browser_datetime::render_locale, datetime::Label};
    let instant = parse("2026-10-18T19:53:21.8939891Z").unwrap();
    for (zone, locale, expected) in [
        (
            "America/New_York",
            "en-US",
            "October 18, 2026, 03:53 PM (UTC-04:00)",
        ),
        (
            "Europe/Berlin",
            "de-DE",
            "18. Oktober 2026, 21:53 (UTC+02:00)",
        ),
        (
            "Europe/Berlin",
            "en-US",
            "October 18, 2026, 09:53 PM (UTC+02:00)",
        ),
        ("Asia/Tokyo", "ja-JP", "2026年10月19日, 04:53 (UTC+09:00)"),
    ] {
        assert_eq!(
            render_locale(Label::at(instant, &named_zone(zone).unwrap()), Some(locale)),
            expected
        );
    }
    // Invalid Intl locale requests fall back without breaking the page.
    assert_eq!(
        render_locale(
            Label::at(instant, &named_zone("UTC").unwrap()),
            Some("invalid_locale!")
        ),
        "2026-10-18, 19:53 UTC+00:00"
    );
}

#[wasm_bindgen_test]
fn api_timestamp_values_reach_locale_display_without_losing_their_instant() {
    use ruphish_domain::datetime::named_zone;
    use ruphish_ui::{
        browser_datetime::render_locale,
        datetime::{Label, instant},
    };

    // The API sends RFC3339 strings for campaign dates/activity, and integer
    // seconds for reporting, delivery attempts and integration history.
    // Both representations must reach the same formatter, even across midnight.
    let response = json!({
        "launch_date": "2026-10-18T19:53:21.8939891Z",
        "activity": [{"time": "2026-10-18T21:53:21.8939891+02:00"}],
        "started": 1792353201
    });
    let zone = named_zone("Asia/Tokyo").unwrap();
    for value in [
        &response["launch_date"],
        &response["activity"][0]["time"],
        &response["started"],
    ] {
        let parsed = instant(value).unwrap().unwrap();
        assert_eq!(
            render_locale(Label::at(parsed, &zone), Some("en-GB")),
            "19 October 2026, 04:53 (UTC+09:00)"
        );
    }
    for value in [
        serde_json::Value::Null,
        json!(""),
        json!("0001-01-01T00:00:00Z"),
    ] {
        assert_eq!(instant(&value).unwrap(), None);
    }
    for value in [
        json!("2026-02-30T19:53:21Z"),
        json!("2026-10-18T19:53:21"),
        json!(i64::MAX),
    ] {
        assert!(instant(&value).is_err());
    }
}

#[wasm_bindgen_test]
fn resource_reads_construct_real_fetch_requests_without_a_body() {
    for method in ["GET", "HEAD"] {
        for family in ["groups", "templates", "pages", "smtp"] {
            let request = resource_request(
                method,
                &format!("https://admin.example.test/api/{family}/1"),
                "synthetic-csrf",
                None,
                None,
            )
            .unwrap();
            assert!(request.body().is_none());
            assert_eq!(request.method().as_str(), method);
            assert_eq!(
                request.headers().get("accept").as_deref(),
                Some("application/json")
            );
        }
    }
    // This reproduces the original failure in the real Fetch implementation.
    assert!(
        gloo_net::http::Request::get("https://admin.example.test/api/groups/1")
            .body("")
            .is_err()
    );
}

#[wasm_bindgen_test]
async fn create_edit_and_delete_preserve_payload_csrf_and_revision() {
    let draft = json!({"id":1,"name":"Équipe","targets":[{"email":"alice@example.test"}]});
    for method in ["POST", "PUT"] {
        let request = resource_request(
            method,
            "https://admin.example.test/api/groups/1",
            "synthetic-csrf",
            Some(&draft),
            Some("\"revision-1\""),
        )
        .unwrap();
        assert_eq!(request.method().as_str(), method);
        assert_eq!(
            request.headers().get("x-csrf-token").as_deref(),
            Some("synthetic-csrf")
        );
        assert_eq!(
            request.headers().get("if-match").as_deref(),
            Some("\"revision-1\"")
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&request.text().await.unwrap()).unwrap(),
            draft
        );
    }
    for family in [
        "groups",
        "templates",
        "pages",
        "smtp",
        "domains",
        "reporting-mailboxes",
        "webhooks",
    ] {
        let delete = resource_request(
            "DELETE",
            &format!("https://admin.example.test/api/{family}/1"),
            "synthetic-csrf",
            None,
            Some("\"revision-1\""),
        )
        .unwrap();
        assert!(delete.body().is_none());
        assert_eq!(delete.method().as_str(), "DELETE");
        assert_eq!(
            delete.headers().get("x-csrf-token").as_deref(),
            Some("synthetic-csrf")
        );
        assert_eq!(
            delete.headers().get("if-match").as_deref(),
            Some("\"revision-1\"")
        );
    }
    assert!(
        resource_request(
            "GET",
            "https://admin.example.test/api/groups/1",
            "",
            Some(&draft),
            None
        )
        .is_err()
    );
    assert!(
        resource_request(
            "INVALID",
            "https://admin.example.test/api/groups/1",
            "",
            None,
            None
        )
        .is_err()
    );
}

#[wasm_bindgen_test]
fn import_draft_handoff_shares_edits_and_isolates_family_session_and_discard() {
    use leptos::prelude::*;
    use ruphish_ui::{
        import_drafts::ImportDrafts, import_state::prepare_creation, resource_state::ResourceDraft,
    };
    let owner = Owner::new();
    let store = owner.with(ImportDrafts::default);
    let imported = prepare_creation(
        "groups",
        "json",
        json!({"name":"Original","targets":[{"email":"alice@example.test"}]}),
    )
    .unwrap();
    let mut draft = ResourceDraft::new(json!({"id":0,"name":"","targets":[]}), None);
    draft.merge_import(&imported.fields);
    let id = store.insert("groups", draft, imported.review).unwrap();
    assert_eq!(id.len(), 36);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    assert!(store.get("templates", &id).is_none());
    assert!(store.get("groups", "unknown").is_none());
    let form_owner = owner.child();
    let form = form_owner.with(|| RwSignal::from(store.get("groups", &id).unwrap().draft));
    form.update(|draft| draft.edit(|v| v["name"] = json!("Reviewed")));
    assert_eq!(
        store
            .get("groups", &id)
            .unwrap()
            .draft
            .with_untracked(|d| d.value()["name"].clone()),
        "Reviewed"
    );
    form_owner.cleanup();
    // Component cleanup does not invalidate the store's shared data before explicit discard.
    assert_eq!(
        store
            .get("groups", &id)
            .unwrap()
            .draft
            .with_untracked(|d| d.value()["name"].clone()),
        "Reviewed"
    );
    let other_owner = Owner::new();
    let other = other_owner.with(ImportDrafts::default);
    assert!(other.get("groups", &id).is_none());
    store.remove(&id);
    assert!(store.get("groups", &id).is_none());
    let imported = prepare_creation("pages", "html", json!({"html":"<p>next</p>"})).unwrap();
    let id = store
        .insert(
            "pages",
            ResourceDraft::new(imported.fields, None),
            imported.review,
        )
        .unwrap();
    owner.cleanup();
    assert!(store.get("pages", &id).is_none());
    other_owner.cleanup();
}

#[wasm_bindgen_test]
fn import_draft_capacity_and_expired_session_fail_without_overwriting_retained_work() {
    use leptos::prelude::*;
    use ruphish_ui::{
        import_drafts::ImportDrafts,
        import_state::{ImportError, prepare_creation},
        resource_state::ResourceDraft,
    };
    let owner = Owner::new();
    let store = owner.with(ImportDrafts::default);
    let insert = || {
        let value = prepare_creation("pages", "html", json!({"html":"<p>retained</p>"})).unwrap();
        store.insert(
            "pages",
            ResourceDraft::new(value.fields, None),
            value.review,
        )
    };
    let mut ids = std::collections::HashSet::new();
    for _ in 0..8 {
        assert!(ids.insert(insert().unwrap()));
    }
    assert_eq!(insert().unwrap_err(), ImportError::Capacity);
    for id in &ids {
        assert!(store.get("pages", id).is_some());
    }
    let removed = ids.iter().next().unwrap();
    store.remove(removed);
    assert!(store.get("pages", removed).is_none());
    assert!(!ids.contains(&insert().unwrap()));
    owner.cleanup();
    assert_eq!(insert().unwrap_err(), ImportError::DraftUnavailable);
}

#[wasm_bindgen_test]
async fn campaign_launch_retry_preserves_reviewed_body_key_and_browser_auth_boundary() {
    let body = json!({"name":"Reviewed campaign","selection":{"template":{"id":9,"revision":"review-v1"}}});
    for _ in 0..2 {
        let request = ruphish_ui::browser_http::launch_request(
            "https://admin.example.test/api/campaigns/",
            "csrf-canary",
            "same-launch-key",
            &body,
        )
        .unwrap();
        assert_eq!(
            request.headers().get("idempotency-key").as_deref(),
            Some("same-launch-key")
        );
        assert_eq!(
            request.headers().get("x-ruphish-client").as_deref(),
            Some("browser")
        );
        assert_eq!(
            request.headers().get("x-csrf-token").as_deref(),
            Some("csrf-canary")
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&request.text().await.unwrap()).unwrap(),
            body
        );
    }
}

#[path = "support/request_lifecycle.rs"]
mod request_lifecycle;
