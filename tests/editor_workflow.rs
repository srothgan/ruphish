//! Template editing-session workflows with independent Go wire input.
//! CodeMirror history/completion workflows run against the real JavaScript packages separately.

#![allow(clippy::unwrap_used)]

use crate::Draft;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;

#[test]
fn go_template_can_be_edited_exported_reopened_and_cancelled_without_losing_content() {
    let bytes = include_bytes!("fixtures/compatibility/editor-template.json");
    let expected: Value = serde_json::from_slice(bytes).unwrap();
    let mut draft = Draft::import(bytes).unwrap();
    let original = draft.value().html.clone();
    draft.edit(|template| template.html = format!("<!-- edited -->{original}"));
    assert!(draft.is_dirty());
    let reopened = Draft::import(&draft.export().unwrap()).unwrap();
    assert_eq!(
        reopened.value().html,
        format!("<!-- edited -->{}", expected["html"].as_str().unwrap())
    );
    assert_eq!(reopened.value().attachments, draft.value().attachments);
    draft.edit(|template| template.html = original.clone());
    assert!(!draft.is_dirty());
    assert_eq!(
        serde_json::from_slice::<Value>(&draft.export().unwrap()).unwrap(),
        expected
    );
    draft.edit(|template| template.html = format!("<!-- edited -->{original}"));
    draft.checkpoint();
    draft.edit(|template| template.html = "unwanted overwrite".into());
    draft.reset();
    assert_eq!(
        draft.value().html,
        format!("<!-- edited -->{}", expected["html"].as_str().unwrap())
    );
}

#[test]
fn attachment_workflow_preserves_binary_bytes_duplicate_unicode_names_and_cancel_behavior() {
    let mut draft = Draft::import(include_bytes!(
        "fixtures/compatibility/editor-template.json"
    ))
    .unwrap();
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
    let reopened = Draft::import(&draft.export().unwrap()).unwrap();
    let attachments = reopened.value().attachments.as_ref().unwrap();
    let binary = attachments
        .iter()
        .find(|file| file.content == "AP+ADQo=")
        .unwrap();
    assert_eq!(
        STANDARD.decode(&binary.content).unwrap(),
        vec![0, 255, 128, 13, 10]
    );
    assert_eq!(binary.mime_type, "application/octet-stream");
    assert!(draft.remove_attachment(1));
    assert!(
        draft
            .value()
            .attachments
            .as_ref()
            .unwrap()
            .iter()
            .any(|file| file.content.is_empty() && file.name == "../fixture-\u{00e9}.bin")
    );
    let before = draft.export().unwrap();
    assert!(!draft.remove_attachment(usize::MAX));
    assert_eq!(draft.export().unwrap(), before);
    draft.reset();
    assert_eq!(draft.export().unwrap(), original);
}

#[test]
fn malformed_import_does_not_replace_the_open_document_and_source_survives_adversarial_content() {
    let mut draft = Draft::import(include_bytes!(
        "fixtures/compatibility/editor-template.json"
    ))
    .unwrap();
    draft.edit(|value| value.name = "Unsaved working template".into());
    for invalid in [
        b"".as_slice(),
        b"\xff\xfe",
        b"{".as_slice(),
        b"[]",
        b"null",
        br#"{"attachments":[{"content":42}]}"#,
    ] {
        let before = draft.export().unwrap();
        assert!(draft.open(invalid).is_err());
        assert_eq!(draft.export().unwrap(), before);
        assert!(draft.is_dirty());
    }
    let source = "<!doctype html>\r\n<form action='{{printf \"%s\" .URL}}'><input name='password' type='password'></form><script>alert(1)</script>\r\n";
    draft.edit(|template| template.html = source.into());
    assert_eq!(
        Draft::import(&draft.export().unwrap())
            .unwrap()
            .value()
            .html,
        source
    );
    let preview = crate::preview_document(&draft.value().html);
    assert!(preview.ends_with(source));
    assert_eq!(draft.value().html, source);
    draft
        .open(include_bytes!(
            "fixtures/compatibility/editor-template.json"
        ))
        .unwrap();
    assert!(!draft.is_dirty());
    assert_eq!(draft.value().name, "Compatibility template");
    // Browser execution/isolation is a separate manual acceptance case, not asserted here.
}
