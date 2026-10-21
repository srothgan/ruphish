//! Exercise the production request builder against the JavaScript Fetch implementation.
#![cfg(target_arch = "wasm32")]
#![allow(clippy::unwrap_used)]

use ruphish_ui::browser_http::resource_request;
use serde_json::json;
use wasm_bindgen_test::wasm_bindgen_test;

#[path = "support/locale_window.rs"]
mod locale_window;

#[path = "support/query_lifecycle.rs"]
mod query_lifecycle;

#[wasm_bindgen_test]
fn language_selection_prefers_saved_choices_then_supported_browser_languages() {
    use ruphish_ui::localization::{Locale, resolve};
    let languages = |values: &[&str]| {
        values
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(resolve(None, &languages(&["de-DE", "en-US"])), Locale::de);
    assert_eq!(resolve(None, &languages(&["fr-FR", "de-AT"])), Locale::de);
    assert_eq!(resolve(None, &languages(&["en-GB", "de-DE"])), Locale::en);
    assert_eq!(resolve(Some("en"), &languages(&["de-DE"])), Locale::en);
    assert_eq!(resolve(Some("de"), &languages(&["en-US"])), Locale::de);
    assert_eq!(resolve(Some("invalid"), &languages(&["de-CH"])), Locale::de);
    assert_eq!(resolve(None, &languages(&["DE-de"])), Locale::de);
    assert_eq!(resolve(None, &languages(&["fr-FR"])), Locale::en);
    assert_eq!(resolve(Some(""), &[]), Locale::en);
}

#[wasm_bindgen_test]
async fn translated_text_updates_in_place_without_replacing_draft_state() {
    locale_window::install();
    use leptos::prelude::*;
    use ruphish_ui::{
        localization::{Locale, provide},
        trs,
    };
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    let (context, label, greeting, draft) = owner.with(|| {
        let context = provide(Locale::en);
        let draft = RwSignal::new("User content: Save changes / Änderungen speichern".to_owned());
        (
            context,
            trs!(ui.save_changes),
            trs!(messages.copy_arg0, arg0 = "API"),
            draft,
        )
    });
    // Allow the provider's initialization effects to settle before a manual choice.
    any_spawner::Executor::tick().await;
    assert_eq!(label.get_untracked(), "Save changes");
    let observed = owner.with(|| Memo::new(move |_| label.get()));
    assert_eq!(observed.get_untracked(), "Save changes");
    context.set_locale(Locale::de);
    assert_eq!(label.get_untracked(), "Änderungen speichern");
    assert_eq!(greeting.get_untracked(), "API kopieren");
    locale_window::change();
    any_spawner::Executor::tick().await;
    assert_eq!(context.get_locale_untracked(), Locale::de);
    assert_eq!(observed.get_untracked(), "Änderungen speichern");
    assert_eq!(
        draft.get_untracked(),
        "User content: Save changes / Änderungen speichern"
    );
    context.set_locale(Locale::en);
    assert_eq!(label.get_untracked(), "Save changes");
    assert_eq!(greeting.get_untracked(), "Copy API");
    owner.cleanup();
}

#[wasm_bindgen_test]
fn localized_observations_preserve_recorded_outcomes_and_external_details() {
    locale_window::install();
    use leptos::prelude::*;
    use ruphish_ui::{
        localization::*,
        recipient_history::{History, timeline},
    };
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    owner.with(|| untrack(|| {
        let context = provide(Locale::en);
        let mut history = History::default();
        let note = ruphish_ui::import_state::ImportNote::Email;
        assert!(import_note(context, &note).contains("Attachments"));
        history.toggle();
        let (ticket, _) = history.begin(false).unwrap();
        history.finish(ticket, Ok(json!({"attempts":[{"generation":2,"started":30,"finished":31,"state":"accepted","smtp_code":null,"stage":""}],"events":[{"id":1,"at":32,"message":"Email Opened"}]})));
        let entries = timeline(&history);
        assert_eq!(timeline_title(context, &entries[1].title), "Attempt 2: Accepted by SMTP server");
        context.set_locale(Locale::de);
        assert!(import_note(context, &note).contains("Anhänge"));
        assert_eq!(timeline_title(context, &entries[1].title), "Versuch 2: Vom SMTP-Server angenommen");
        assert_eq!(timeline_title(context, &entries[2].title), "E-Mail geöffnet");
        assert_eq!(event_label(context, "An external diagnostic"), "An external diagnostic");
        assert!(entries.iter().all(|entry| {
            let text = timeline_title(context, &entry.title);
            !text.contains("null") && !text.contains("250")
        }));
        let error = ruphish_ui::import_state::ImportError::RequestFailed("External response".into());
        assert_eq!(import_error(context, &error), "External response (IMPORT_REQUEST_FAILED)");
        assert_eq!(entries[1].at, Some(31));
        assert_eq!(entries[1].tone, "success");
    }));
    owner.cleanup();
}

#[wasm_bindgen_test]
fn timestamps_use_jiff_zones_and_browser_locales_without_double_conversion() {
    use ruphish_domain::datetime::{named_zone, parse};
    use ruphish_ui::{browser_datetime::render_locale, datetime::Label};
    let instant = parse("2026-10-18T19:53:21.8939891Z").unwrap();
    for (zone, locale, expected) in [
        (
            "America/New_York",
            "en-US",
            "10/18/2026, 03:53 PM (UTC-04:00)",
        ),
        ("Europe/Berlin", "de-DE", "18.10.2026, 21:53 (UTC+02:00)"),
        ("Europe/Berlin", "en-US", "10/18/2026, 09:53 PM (UTC+02:00)"),
        ("Asia/Tokyo", "ja-JP", "2026/10/19 04:53 (UTC+09:00)"),
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
            "19/10/2026, 04:53 (UTC+09:00)"
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

#[wasm_bindgen_test]
fn account_preferences_change_text_and_format_independently_without_changing_instants() {
    locale_window::install();
    use leptos::prelude::*;
    use ruphish_contracts::auth::Preferences;
    use ruphish_domain::datetime;
    use ruphish_ui::{
        datetime::Label,
        display_preferences::DisplayPreferences,
        localization::{self, Locale},
    };
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    owner.with(|| {
        let context = localization::provide(Locale::en);
        let state = DisplayPreferences::provide();
        let instant = datetime::parse("2026-10-18T19:53:21.8939891Z").unwrap();
        let preview = Memo::new(move |_| state.render(Label::at(instant, &state.zone())));
        let heading = ruphish_ui::trs!(preferences.heading);
        let value = Preferences {
            language: Some("de".into()),
            format_locale: Some("zh-CN".into()),
            time_zone: Some("Europe/Berlin".into()),
            show_utc_offset: true,
        };
        state.apply(value.clone(), context);
        assert_eq!(heading.get_untracked(), "Sprache, Datum und Uhrzeit");
        assert_eq!(preview.get_untracked(), "2026/10/18 21:53 (UTC+02:00)");
        state.apply(
            Preferences {
                language: Some("en".into()),
                ..value.clone()
            },
            context,
        );
        assert_eq!(heading.get_untracked(), "Language, date and time");
        assert_eq!(preview.get_untracked(), "2026/10/18 21:53 (UTC+02:00)");
        state.apply(
            Preferences {
                language: Some("en".into()),
                format_locale: Some("de-DE".into()),
                ..value.clone()
            },
            context,
        );
        assert_eq!(heading.get_untracked(), "Language, date and time");
        assert_eq!(preview.get_untracked(), "18.10.2026, 21:53 (UTC+02:00)");
        state.apply(
            Preferences {
                time_zone: Some("Asia/Shanghai".into()),
                ..value
            },
            context,
        );
        assert_eq!(preview.get_untracked(), "2026/10/19 03:53 (UTC+08:00)");
        let mut hidden_offset = state.value.get_untracked();
        hidden_offset.show_utc_offset = false;
        state.apply(hidden_offset, context);
        assert_eq!(preview.get_untracked(), "2026/10/19 03:53");
        assert_eq!(datetime::utc(instant), "2026-10-18T19:53:21.8939891Z");
        state.apply(Preferences::default(), context);
        assert_eq!(
            context.get_locale_untracked(),
            localization::browser_locale()
        );
        assert_eq!(
            datetime::zone_name(&state.zone()),
            datetime::zone_name(&ruphish_ui::browser_datetime::detected_zone())
        );
        assert_eq!(
            preview.get_untracked(),
            ruphish_ui::browser_datetime::render(Label::at(instant, &state.zone()))
        );
    });
    owner.cleanup();
}

#[wasm_bindgen_test]
fn preference_picker_choices_are_accepted_by_the_authoritative_domain() {
    use ruphish_domain::{datetime, preferences::Preferences};
    let zones = datetime::available_zones();
    assert!(zones.len() > 300);
    for expected in ["UTC", "Europe/Berlin", "Asia/Shanghai", "America/New_York"] {
        assert!(zones.iter().any(|zone| zone == expected));
    }
    for zone in zones {
        Preferences {
            time_zone: Some(zone),
            ..Default::default()
        }
        .validate()
        .unwrap();
    }
    let formats = ruphish_ui::browser_datetime::format_locale_options(
        "de",
        "zh-CN",
        ruphish_ui::datetime::Label::at(
            datetime::parse("2026-10-18T19:53:00Z").unwrap(),
            &datetime::named_zone("Europe/Berlin").unwrap(),
        ),
    );
    assert!(formats.len() > 100);
    assert!(
        formats
            .iter()
            .any(|(id, label)| id == "zh-CN" && label.contains("Chinesisch"))
    );
    for (locale, _) in formats {
        Preferences {
            format_locale: Some(locale),
            ..Default::default()
        }
        .validate()
        .unwrap();
    }
}

#[wasm_bindgen_test]
fn received_error_codes_translate_to_plain_text_with_a_stable_support_identifier() {
    use leptos::prelude::*;
    use ruphish_ui::localization::{self, Locale};
    let owner = Owner::new();
    owner.with(|| {
        let context = localization::provide(Locale::de);
        let text = localization::failure_label(context, "AUTH_INVALID_CREDENTIALS");
        assert_eq!(
            text,
            "AUTH_INVALID_CREDENTIALS - Benutzername oder Passwort ungültig."
        );
        context.set_locale(Locale::en);
        assert_eq!(
            localization::failure_label(context, "AUTH_INVALID_CREDENTIALS"),
            "AUTH_INVALID_CREDENTIALS - Invalid username or password."
        );
        assert_eq!(
            text,
            "AUTH_INVALID_CREDENTIALS - Benutzername oder Passwort ungültig."
        );
        assert_eq!(
            localization::failure_label(context, "FUTURE_ERROR"),
            "FUTURE_ERROR - The request failed. Try again or contact your administrator."
        );
        assert_eq!(
            localization::failure_label(context, "CSV_ROW_COLUMNS"),
            "CSV_ROW_COLUMNS - Column count does not match the header."
        );
    });
}

#[wasm_bindgen_test]
fn late_session_preferences_cannot_undo_a_confirmed_save() {
    locale_window::install();
    use leptos::prelude::*;
    use ruphish_contracts::auth::Preferences;
    use ruphish_ui::{
        display_preferences::DisplayPreferences,
        localization::{self, Locale},
    };
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    owner.with(|| {
        let context = localization::provide(Locale::en);
        let state = DisplayPreferences::provide();
        let before_save = state.read_ticket();
        let saved = Preferences {
            language: Some("de".into()),
            time_zone: Some("Europe/Berlin".into()),
            show_utc_offset: false,
            ..Default::default()
        };
        state.apply(saved.clone(), context);
        state.apply_read(before_save, Preferences::default(), context);
        state.read_failed(before_save, "obsolete failure".into());
        assert!(state.error.get_untracked().is_none());
        assert_eq!(state.value.get_untracked(), saved);
        assert_eq!(context.get_locale_untracked(), Locale::de);
        let after_save = state.read_ticket();
        state.read_failed(after_save, "offline".into());
        assert_eq!(state.value.get_untracked(), saved);
        assert_eq!(state.error.get_untracked().as_deref(), Some("offline"));
        state.apply_read(after_save, Preferences::default(), context);
        assert_eq!(state.value.get_untracked(), Preferences::default());
        assert!(state.error.get_untracked().is_none());
        assert_eq!(
            context.get_locale_untracked(),
            localization::browser_locale()
        );
    });
    owner.cleanup();
}
