//! Template editing-session workflows with independent Go wire input.
//! CodeMirror history/completion workflows run against the real JavaScript packages separately.

#![allow(clippy::unwrap_used)]

use crate::resource_state::{ResourceDraft, import_resource};

#[test]
fn create_and_edit_finish_only_after_a_confirmed_save_without_newer_changes() {
    let mut draft = ResourceDraft::new(serde_json::json!({"id":0,"name":"","targets":[]}), None);
    draft.edit(|value| *value=serde_json::json!({"id":0,"name":"Équipe","targets":[{"email":"alice@example.test"}]}));
    let submitted = draft.value().clone();
    // Validation, network and conflict failures never call saved, so the draft stays open.
    assert_eq!(draft.completed_id(), None);
    assert_eq!(draft.value()["name"], "Équipe");
    let mut returned = submitted.clone();
    returned["id"] = serde_json::json!(42);
    draft.saved(&submitted, returned.clone(), Some("first".into()));
    assert_eq!(draft.completed_id(), Some(42));
    draft.edit(|value| value["name"] = serde_json::json!("Renamed"));
    let submitted = draft.value().clone();
    draft.edit(|value| value["targets"][0]["email"] = serde_json::json!("bob@example.test"));
    draft.saved(&submitted, submitted.clone(), Some("second".into()));
    assert_eq!(
        draft.completed_id(),
        None,
        "A late save must not redirect away from newer edits"
    );
    assert_eq!(draft.value()["targets"][0]["email"], "bob@example.test");
    let final_submission = draft.value().clone();
    draft.saved(
        &final_submission,
        final_submission.clone(),
        Some("third".into()),
    );
    assert_eq!(draft.completed_id(), Some(42));
    draft.edit(|value| value["name"] = serde_json::json!("Discard this edit"));
    draft.cancel();
    assert_eq!(draft.completed_id(), Some(42));
    assert_eq!(draft.value()["name"], "Renamed");
}

fn open(bytes: &[u8]) -> ResourceDraft {
    ResourceDraft::new(import_resource(bytes, "templates").unwrap(), None)
}
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;

#[test]
fn go_template_can_be_edited_exported_reopened_and_cancelled_without_losing_content() {
    let bytes = include_bytes!("fixtures/compatibility/editor-template.json");
    let expected: Value = serde_json::from_slice(bytes).unwrap();
    let mut draft = open(bytes);
    let original = draft.value()["html"].as_str().unwrap().to_owned();
    draft.edit(|template| {
        template["html"] = serde_json::json!(format!("<!-- edited -->{original}"))
    });
    assert!(draft.dirty());
    let reopened = open(&draft.export().unwrap());
    assert_eq!(
        reopened.value()["html"],
        format!("<!-- edited -->{}", expected["html"].as_str().unwrap())
    );
    assert_eq!(
        reopened.value()["attachments"],
        draft.value()["attachments"]
    );
    draft.edit(|template| template["html"] = serde_json::json!(original.clone()));
    assert!(!draft.dirty());
    assert_eq!(
        serde_json::from_slice::<Value>(&draft.export().unwrap()).unwrap(),
        expected
    );
    draft.edit(|template| {
        template["html"] = serde_json::json!(format!("<!-- edited -->{original}"))
    });
    let submitted = draft.value().clone();
    draft.saved(&submitted, submitted.clone(), Some("saved".into()));
    draft.edit(|template| template["html"] = serde_json::json!("unwanted overwrite"));
    draft.cancel();
    assert_eq!(
        draft.value()["html"],
        format!("<!-- edited -->{}", expected["html"].as_str().unwrap())
    );
}

#[test]
fn attachment_workflow_preserves_binary_bytes_duplicate_unicode_names_and_cancel_behavior() {
    let mut draft = open(include_bytes!(
        "fixtures/compatibility/editor-template.json"
    ));
    let original = draft.export().unwrap();
    draft.attach(
        "../fixture-\u{00e9}.bin".into(),
        String::new(),
        &[0, 255, 128, 13, 10],
    );
    draft.attach(
        "../fixture-\u{00e9}.bin".into(),
        "application/octet-stream".into(),
        &[],
    );
    let reopened = open(&draft.export().unwrap());
    let attachments = reopened.value()["attachments"].as_array().unwrap();
    let binary = attachments
        .iter()
        .find(|file| file["content"] == "AP+ADQo=")
        .unwrap();
    assert_eq!(
        STANDARD
            .decode(binary["content"].as_str().unwrap())
            .unwrap(),
        vec![0, 255, 128, 13, 10]
    );
    assert_eq!(binary["type"], "application/octet-stream");
    assert!(draft.remove_attachment(1));
    assert!(
        draft.value()["attachments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["content"] == "" && file["name"] == "../fixture-\u{00e9}.bin")
    );
    let before = draft.export().unwrap();
    assert!(!draft.remove_attachment(usize::MAX));
    assert_eq!(draft.export().unwrap(), before);
    draft.cancel();
    assert_eq!(draft.export().unwrap(), original);
}

#[test]
fn malformed_import_does_not_replace_the_open_document_and_source_survives_adversarial_content() {
    let mut draft = open(include_bytes!(
        "fixtures/compatibility/editor-template.json"
    ));
    draft.edit(|value| value["name"] = serde_json::json!("Unsaved working template"));
    for invalid in [
        b"".as_slice(),
        b"\xff\xfe",
        b"{".as_slice(),
        b"[]",
        b"null",
        br#"{"attachments":[{"content":42}]}"#,
    ] {
        let before = draft.export().unwrap();
        assert!(draft.import(invalid, "templates").is_err());
        assert_eq!(draft.export().unwrap(), before);
        assert!(draft.dirty());
    }
    let source = "<!doctype html>\r\n<form action='{{printf \"%s\" .URL}}'><input name='password' type='password'></form><script>alert(1)</script>\r\n";
    draft.edit(|template| template["html"] = serde_json::json!(source));
    assert_eq!(open(&draft.export().unwrap()).value()["html"], source);
    let preview = crate::preview_document(draft.value()["html"].as_str().unwrap());
    assert!(preview.ends_with(source));
    assert_eq!(draft.value()["html"], source);
    draft
        .import(
            include_bytes!("fixtures/compatibility/editor-template.json"),
            "templates",
        )
        .unwrap();
    assert!(!draft.dirty());
    assert_eq!(draft.value()["name"], "Compatibility template");
    // Browser execution/isolation is a separate manual acceptance case, not asserted here.
}

#[test]
fn imported_resources_keep_supported_fields_but_never_inherit_identity() {
    use crate::import_state::prepare_creation;
    use serde_json::json;
    let cases = [
        (
            "groups",
            json!({"name":"Team","targets":[{"id":88,"user_id":9,"email":"alice@example.test","first_name":"Alice","last_name":"Example","position":"Analyst"}]}),
        ),
        (
            "templates",
            json!({"name":"Mail","subject":"Hi {{.FirstName}}","envelope_sender":"sender@example.test","html":"<p>?</p>","text":"?","attachments":[{"name":"guide.bin","type":"application/octet-stream","content":"AP8="}]}),
        ),
        (
            "pages",
            json!({"name":"Page","html":"<form><input name='name'></form>","redirect_url":"https://example.test/done"}),
        ),
        (
            "smtp",
            json!({"name":"Mail server","interface_type":"SMTP","host":"smtp.example.test:587","from_address":"training@example.test","username":"trainer","password":"synthetic-secret","ignore_cert_errors":false,"headers":[{"id":88,"key":"X-Test","value":"{{.FirstName}}"}]}),
        ),
    ];
    for (family, fields) in cases {
        let mut source = fields.clone();
        source["id"] = json!(91);
        source["user_id"] = json!(9);
        source["modified_date"] = json!("old-format-not-relevant");
        source["unknown"] = json!("must not persist");
        let imported = prepare_creation(family, "json", source).unwrap();
        assert_eq!(imported.fields["id"], 0);
        for key in ["user_id", "modified_date", "unknown"] {
            assert!(imported.fields.get(key).is_none());
        }
        for (key, expected) in fields.as_object().unwrap() {
            if key != "targets" && key != "headers" {
                assert_eq!(&imported.fields[key], expected);
            }
        }
        for key in ["targets", "headers"] {
            for child in imported.fields[key].as_array().into_iter().flatten() {
                assert!(child.get("id").is_none());
                assert!(child.get("user_id").is_none());
            }
        }
        let mut draft = crate::resource_state::ResourceDraft::new(json!({"id":0,"name":""}), None);
        draft.merge_import(&imported.fields);
        assert!(draft.dirty());
        assert_eq!(draft.completed_id(), None);
        draft.edit(|v| v["name"] = json!("Reviewed name"));
        let submitted = draft.value().clone();
        // A failed save does not acknowledge or replace the current draft.
        assert!(draft.dirty());
        assert_eq!(draft.value()["name"], "Reviewed name");
        let mut saved = submitted.clone();
        saved["id"] = json!(123);
        draft.saved(&submitted, saved, Some("new-revision".into()));
        assert_eq!(draft.completed_id(), Some(123));
    }
}

#[test]
fn csv_import_requires_review_of_omissions_and_retains_the_error_report() {
    use crate::import_state::prepare_creation;
    use serde_json::json;
    let error = json!({"row":4,"message":"Email is required"});
    let mut imported = prepare_creation(
        "groups",
        "csv",
        json!({"targets":[
        {"email":"Alice@example.test","first_name":"First"},
        {"email":"alice@example.test","first_name":"Second"},
        {"email":"bob@example.test"}],"errors":[error.clone()]}),
    )
    .unwrap();
    assert_eq!(imported.fields["targets"].as_array().unwrap().len(), 2);
    assert_eq!(imported.fields["targets"][0]["first_name"], "First");
    assert_eq!(imported.review.rejected_rows, vec![error]);
    assert!(!imported.review.ready());
    imported.review.acknowledged = true;
    assert!(imported.review.ready());
    assert!(prepare_creation("groups", "csv", json!({"targets":[],"errors":[]})).is_err());
    let lossless = prepare_creation(
        "groups",
        "csv",
        json!({"targets":[{"email":"a@example.test"}],"errors":[]}),
    )
    .unwrap();
    assert!(lossless.review.ready());
    // JSON preserves API address identity instead of imposing CSV deduplication.
    let json_import = prepare_creation(
        "groups",
        "json",
        json!({"targets":[{"email":"A@example.test"},{"email":"a@example.test"}]}),
    )
    .unwrap();
    assert_eq!(json_import.fields["targets"].as_array().unwrap().len(), 2);
}

#[test]
fn incomplete_content_imports_remain_drafts_and_invalid_imports_are_rejected() {
    use crate::import_state::prepare_creation;
    use serde_json::json;
    let mail = prepare_creation(
        "templates",
        "email",
        json!({"subject":"Bonjour","html":"<p>? {{.FirstName}}</p>","text":"Bonjour"}),
    )
    .unwrap();
    assert_eq!(mail.fields["name"], "");
    assert_eq!(mail.fields["envelope_sender"], "");
    assert_eq!(mail.fields["attachments"], json!([]));
    assert!(
        mail.review
            .notes
            .iter()
            .any(|note| matches!(note, crate::import_state::ImportNote::Email))
    );
    let page = prepare_creation(
        "pages",
        "html",
        json!({"html":"<form><input name='password'></form>"}),
    )
    .unwrap();
    assert!(page.fields.get("capture_passwords").is_none());
    assert_eq!(page.fields["redirect_url"], "");
    for (family, format, input) in [
        ("smtp", "json", json!({"targets":[]})),
        ("templates", "json", json!({"html":42})),
        ("groups", "json", json!({"targets":"not rows"})),
        ("pages", "email", json!({"html":"x"})),
        ("pages", "json", json!(null)),
    ] {
        assert!(prepare_creation(family, format, input).is_err());
    }
}

#[test]
fn import_failures_report_stable_codes_without_echoing_sensitive_input() {
    use crate::import_state::{ImportError, parse_json, prepare_creation};
    use serde_json::json;
    let secret = "synthetic-secret-never-echo";
    let invalid = parse_json(&format!("{{\"password\":\"{secret}\"")).unwrap_err();
    assert_eq!(invalid.code(), "IMPORT_INVALID_JSON");
    assert!(!invalid.to_string().contains(secret));
    assert_eq!(
        parse_json(&"x".repeat(8 * 1024 * 1024 + 1)).unwrap_err(),
        ImportError::TooLarge
    );
    for (family, format, input, code) in [
        (
            "smtp",
            "json",
            json!({"host":42,"password":secret}),
            "IMPORT_WRONG_RESOURCE_TYPE",
        ),
        (
            "groups",
            "html",
            json!({"html":"x"}),
            "IMPORT_UNSUPPORTED_FORMAT",
        ),
        (
            "groups",
            "csv",
            json!({"targets":[],"errors":[]}),
            "IMPORT_NO_VALID_RECIPIENTS",
        ),
    ] {
        let error = prepare_creation(family, format, input).err().unwrap();
        assert_eq!(error.code(), code);
        assert!(error.to_string().contains(code));
        assert!(!error.to_string().contains(secret));
    }
}

#[test]
fn batch_email_json_is_rejected_as_a_whole_while_group_recipient_arrays_are_supported() {
    use crate::import_state::{ImportError, parse_json, prepare_creation};
    use serde_json::json;
    for source in [
        r#"[{"name":"First","subject":"One","html":"<p>one</p>"},{"name":"Second","subject":"Two","html":"<p>two</p>"}]"#,
        r#"{"templates":[{"name":"First","html":"<p>one</p>"},{"name":"Second","html":"<p>two</p>"}]}"#,
        r#"{"name":"Invalid","subject":["One","Two"],"html":"<p>one</p>"}"#,
        r#"{"name":"Invalid","html":["<p>one</p>","<p>two</p>"]}"#,
    ] {
        let input = parse_json(source).unwrap();
        let error = prepare_creation("templates", "json", input).err().unwrap();
        let batch = source.starts_with('[') || source.contains("\"templates\"");
        assert_eq!(
            error.code(),
            if batch {
                "IMPORT_BATCH_NOT_SUPPORTED"
            } else {
                "IMPORT_WRONG_RESOURCE_TYPE"
            }
        );
        if batch {
            assert_eq!(error, ImportError::BatchNotSupported("email template"));
        } else {
            assert_eq!(error, ImportError::WrongResourceType);
        }
    }
    let group = prepare_creation(
        "groups",
        "json",
        json!({"name":"Team","targets":[{"email":"a@example.test"},{"email":"b@example.test"}]}),
    )
    .unwrap();
    assert_eq!(group.fields["targets"][0]["email"], "a@example.test");
    assert_eq!(group.fields["targets"][1]["email"], "b@example.test");
    assert_eq!(group.fields["id"], 0);
}

#[test]
fn every_resource_import_rejects_batches_but_accepts_nested_collection_fields() {
    use crate::import_state::{ImportError, prepare_creation};
    use serde_json::json;
    for (family, singular, resource) in [
        (
            "groups",
            "group",
            json!({"name":"Team","targets":[{"email":"a@example.test"},{"email":"b@example.test"}]}),
        ),
        (
            "templates",
            "email template",
            json!({"name":"Mail","html":"<p>hello</p>","attachments":[{"name":"a.txt","type":"text/plain","content":"YQ=="},{"name":"b.txt","type":"text/plain","content":"Yg=="}]}),
        ),
        (
            "pages",
            "landing page",
            json!({"name":"Page","html":"<p>hello</p>"}),
        ),
        (
            "smtp",
            "sending profile",
            json!({"name":"Profile","host":"smtp.example.test","headers":[{"key":"X-One","value":"1"},{"key":"X-Two","value":"2"}]}),
        ),
    ] {
        assert!(prepare_creation(family, "json", resource.clone()).is_ok());
        for batch in [
            json!([resource.clone(), resource.clone()]),
            json!({family:[resource.clone(),resource.clone()]}),
        ] {
            let error = prepare_creation(family, "json", batch).err().unwrap();
            assert_eq!(error, ImportError::BatchNotSupported(singular));
            assert_eq!(error.code(), "IMPORT_BATCH_NOT_SUPPORTED");
        }
        // A valid-looking root must not cause an embedded batch to be silently ignored.
        let mut mixed = resource.clone();
        mixed[family] = json!([resource]);
        assert_eq!(
            prepare_creation(family, "json", mixed)
                .err()
                .unwrap()
                .code(),
            "IMPORT_BATCH_NOT_SUPPORTED"
        );
    }
}
