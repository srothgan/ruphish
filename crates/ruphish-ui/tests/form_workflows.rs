//! Functional editing scenarios using the same projections and state as the WASM forms.
#![allow(clippy::unwrap_used)]
use ruphish_contracts::validation::{Code, ValidationReport};
use ruphish_domain::validation::{Issue, Report};
use ruphish_ui::{
    api_error, form_state::FormState, form_validation, resource_state::ResourceDraft,
};
use serde_json::json;

fn fields(report: &Report) -> Vec<&str> {
    report
        .issues()
        .iter()
        .map(|issue| issue.field.as_str())
        .collect()
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn resource_correction_server_conflict_and_late_failure_preserve_the_current_edit() {
    let mut draft = ResourceDraft::new(json!({"id":0,"name":"","text":"","html":""}), None);
    let mut form = FormState::default();
    let initial = form_validation::resource(draft.value(), "templates");
    assert!(
        form.issues(initial.issues()).is_empty(),
        "untouched creation stays quiet"
    );
    form.submit(initial.issues());
    assert_eq!(fields(&initial), ["name", "html", "text"]);
    draft.edit(|v| {
        v["name"] = json!("Training");
        v["text"] = json!("Hello {{.FirstName}}");
    });
    form.edit("name");
    form.edit("text");
    assert!(form_validation::resource(draft.value(), "templates").is_empty());
    let ticket = form.ticket();
    let rejected = api_error::response(409, Ok(json!({"success":false,"code":"RESOURCE_NAME_TAKEN","message":"untrusted", "validation":{"issues":[{"field":"name","code":"conflict"}]}})), str::to_owned).unwrap_err();
    assert_eq!(
        rejected.status, 409,
        "conflict classification is independent of field feedback"
    );
    assert!(form.receive(ticket, rejected.validation.unwrap()));
    assert_eq!(form.field(&[], "name"), Some(Code::Conflict));
    form.submit(&[]);
    assert!(
        form.issues(&[]).is_empty(),
        "retry replaces the previous server verdict"
    );
    assert!(form.receive(
        ticket,
        ValidationReport {
            issues: vec![Issue {
                field: "name".into(),
                code: Code::Conflict
            }]
        }
    ));
    draft.edit(|v| v["name"] = json!("Different training"));
    form.edit("name");
    assert!(form.issues(&[]).is_empty());
    assert!(!form.receive(
        ticket,
        ValidationReport {
            issues: vec![Issue {
                field: "name".into(),
                code: Code::Conflict
            }]
        }
    ));
    let submitted = draft.value().clone();
    draft.edit(|v| v["text"] = json!("A newer unsaved revision"));
    form.edit("text");
    let mut saved = submitted.clone();
    saved["id"] = json!(7);
    saved["modified_date"] = json!("2026-10-25T12:00:00Z");
    draft.saved(&submitted, saved, Some("revision-1".into()));
    assert_eq!(draft.value()["text"], "A newer unsaved revision");
    assert!(
        draft.completed_id().is_none(),
        "a late save must not navigate away from newer work"
    );
    let latest = draft.value().clone();
    draft.saved(&latest, latest.clone(), Some("revision-2".into()));
    assert_eq!(draft.completed_id(), Some(7));
    assert_eq!(draft.value(), &latest);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn incomplete_campaign_reopened_on_review_requires_correction_but_remains_saveable() {
    let now = ruphish_domain::datetime::parse("2026-10-25T12:00:00Z")
        .unwrap()
        .as_second();
    let mut value = json!({"name":"","url":"","selection":{"groups":[],"template":{"id":0},"page":{"id":0},"smtp":{"id":0}},"policy":{"redirect_url":""}});
    let saved: ruphish_domain::campaign_draft::Configuration =
        serde_json::from_value(value.clone()).unwrap();
    assert!(
        saved.validate(),
        "incomplete data is a valid persisted draft"
    );
    let reopened = serde_json::to_value(saved).unwrap();
    let review = form_validation::campaign(&reopened, 3, now);
    assert_eq!(
        fields(&review),
        [
            "name",
            "selection.groups",
            "selection.template.id",
            "selection.page.id",
            "selection.smtp.id",
            "url"
        ]
    );
    assert_eq!(
        ruphish_domain::campaign_draft::field_step(&review.issues()[0].field),
        0
    );
    value["name"] = json!("Training");
    value["selection"]["groups"] = json!([{"id":1}]);
    assert!(form_validation::campaign(&value, 0, now).is_empty());
    assert!(!form_validation::campaign(&value, 1, now).is_empty());
    value["selection"]["template"]["id"] = json!(2);
    value["selection"]["page"]["id"] = json!(3);
    value["selection"]["smtp"]["id"] = json!(4);
    value["url"] = json!("https://training.example.test/path");
    assert!(
        form_validation::campaign(&value, 3, now).is_empty(),
        "immediate delivery needs no explicit instant"
    );
    value["launch_local"] = json!({"local":"","time_zone":"Europe/Berlin"});
    assert!(
        serde_json::from_value::<ruphish_domain::campaign_draft::Configuration>(value.clone())
            .unwrap()
            .validate()
    );
    assert_eq!(
        fields(&form_validation::campaign(&value, 3, now)),
        ["launch_local.local"]
    );
    value["launch_local"]["local"] = json!("2026-10-26T12:00");
    value["launch_local"]["time_zone"] = json!("not-a-zone");
    assert_eq!(
        fields(&form_validation::campaign(&value, 3, now)),
        ["launch_local.time_zone"]
    );
    value["launch_local"]["time_zone"] = json!("Europe/Berlin");
    value["send_by_local"] = json!({"local":"2026-10-25T12:00","time_zone":"Europe/Berlin"});
    assert_eq!(
        form_validation::campaign(&value, 3, now).issues()[0].code,
        Code::ScheduleOrder
    );
    value["send_by_local"]["local"] = json!("2026-10-26T15:00");
    assert!(form_validation::campaign(&value, 3, now).is_empty());
    let later = ruphish_domain::datetime::parse("2026-10-27T12:00:00Z")
        .unwrap()
        .as_second();
    assert_eq!(
        form_validation::campaign(&value, 3, later).issues()[0].code,
        Code::SchedulePast
    );
    value["launch_local"] = serde_json::Value::Null;
    value["send_by_local"] = serde_json::Value::Null;
    assert!(form_validation::campaign(&value, 3, later).is_empty());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn credentials_and_operational_failures_do_not_become_interchangeable_validation_errors() {
    let mut form = FormState::default();
    let report = ruphish_domain::password::replacement("tiny", "different");
    form.submit(report.issues());
    assert_eq!(form.issues(report.issues()).len(), 2);
    assert!(!format!("{report:?}").contains("tiny"));
    assert!(!format!("{report:?}").contains("different"));
    assert!(ruphish_domain::password::replacement("éééé", "éééé").is_empty());
    assert!(
        !ruphish_domain::password::replacement(&"x".repeat(1025), &"x".repeat(1025)).is_empty()
    );
    let mut hook = json!({"name":"Hook","url":"https://example.test/events","secret":""});
    assert_eq!(
        fields(&form_validation::integration(&hook, true, false)),
        ["secret"]
    );
    assert!(
        form_validation::integration(&hook, true, true).is_empty(),
        "blank edit preserves saved secret"
    );
    hook["secret"] = json!("synthetic-secret");
    hook["url"] = json!("https://user:password@example.test/#fragment");
    let report = form_validation::integration(&hook, true, false);
    assert_eq!(fields(&report), ["url"]);
    assert!(!format!("{report:?}").contains("synthetic-secret"));
    for status in [401, 403, 412, 429, 503] {
        let failure = api_error::response(
            status,
            Ok(json!({"success":false,"message":"untrusted"})),
            str::to_owned,
        )
        .unwrap_err();
        assert_eq!(failure.status, status);
        assert!(failure.validation.is_none());
    }
    let unknown = api_error::response(
        400,
        Ok(json!({"validation":{"issues":[{"field":"name","code":"future_rule"}]}})),
        str::to_owned,
    )
    .unwrap_err();
    assert_eq!(unknown.validation.unwrap().issues[0].code, Code::Unknown);
    let unsafe_details = api_error::response(400, Ok(json!({"validation":{"issues":[{"field":"password","code":"invalid","value":"never-display"}]}})), str::to_owned).unwrap_err();
    assert!(
        unsafe_details.validation.is_none(),
        "unexpected value-bearing payload is discarded"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn campaign_step_jumps_require_skipped_content_and_recheck_resource_availability() {
    let now = ruphish_domain::datetime::parse("2026-10-25T12:00:00Z")
        .unwrap()
        .as_second();
    let mut draft = json!({"name":"Training","url":"","selection":{"groups":[{"id":1}],"template":{"id":0},"page":{"id":0},"smtp":{"id":0}},"policy":{"redirect_url":""}});
    let mut resources = json!({"groups":[{"id":1}],"template":[{"id":2}],"page":[{"id":3}],"smtp":[{"id":4}],"learning_page":[{"id":5}],"domains":[{"origin":"https://training.example.test"}]});
    let readiness =
        |draft: &serde_json::Value, resources: &serde_json::Value, destination: usize| {
            form_validation::campaign_with_resources(
                draft,
                Some(resources),
                destination.saturating_sub(1),
                now,
            )
        };
    assert!(
        readiness(&draft, &resources, 1).is_empty(),
        "audience permits entering content"
    );
    assert_eq!(
        fields(&readiness(&draft, &resources, 2)),
        [
            "selection.template.id",
            "selection.page.id",
            "selection.smtp.id",
            "url"
        ],
        "jumping directly to schedule cannot bypass content"
    );
    assert!(!readiness(&draft, &resources, 3).is_empty());
    draft["selection"]["template"]["id"] = json!(2);
    draft["selection"]["page"]["id"] = json!(3);
    draft["selection"]["smtp"]["id"] = json!(4);
    draft["url"] = json!("https://training.example.test");
    assert!(
        readiness(&draft, &resources, 3).is_empty(),
        "complete immediate campaign permits direct review"
    );
    draft["launch_local"] = json!({"local":"","time_zone":"UTC"});
    assert!(
        readiness(&draft, &resources, 2).is_empty(),
        "schedule can be entered to complete it"
    );
    assert_eq!(
        fields(&readiness(&draft, &resources, 3)),
        ["launch_local.local"]
    );
    draft["launch_local"]["local"] = json!("2026-10-26T12:00");
    assert!(readiness(&draft, &resources, 3).is_empty());
    resources["template"] = json!([]);
    let missing = readiness(&draft, &resources, 3);
    assert_eq!(fields(&missing), ["selection.template.id"]);
    assert_eq!(missing.issues()[0].code, Code::Unavailable);
    resources["template"] = json!([{"id":2}]);
    draft["policy"]["learning_page_id"] = json!(6);
    assert_eq!(
        fields(&readiness(&draft, &resources, 3)),
        ["policy.learning_page_id"]
    );
    draft["policy"]["learning_page_id"] = json!(5);
    assert!(readiness(&draft, &resources, 3).is_empty());
    resources["domains"] = json!([]);
    let missing = readiness(&draft, &resources, 3);
    assert_eq!(fields(&missing), ["url"]);
    assert_eq!(missing.issues()[0].code, Code::Unavailable);
    resources["domains"] = json!([{"origin":"https://training.example.test"}]);
    draft["url"] = json!("https://training.example.test/training");
    assert!(readiness(&draft, &resources, 3).is_empty());
    draft["selection"]["groups"] = json!([]);
    assert_eq!(
        fields(&readiness(&draft, &resources, 3)),
        ["selection.groups"]
    );
    assert!(
        serde_json::from_value::<ruphish_domain::campaign_draft::Configuration>(draft)
            .unwrap()
            .validate(),
        "an incomplete edit remains saveable as a draft"
    );
}
