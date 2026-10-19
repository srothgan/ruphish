//! Exercise the production request builder against the JavaScript Fetch implementation.
#![cfg(target_arch = "wasm32")]
#![allow(clippy::unwrap_used)]

use ruphish_ui::browser_http::resource_request;
use serde_json::json;
use wasm_bindgen_test::wasm_bindgen_test;

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
    let delete = resource_request(
        "DELETE",
        "https://admin.example.test/api/groups/1",
        "synthetic-csrf",
        None,
        Some("\"revision-1\""),
    )
    .unwrap();
    assert!(delete.body().is_none());
    assert_eq!(delete.method().as_str(), "DELETE");
    assert_eq!(
        delete.headers().get("if-match").as_deref(),
        Some("\"revision-1\"")
    );
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
