//! Recipient draft and unchanged HTTP contract guarantees.
#![allow(clippy::unwrap_used, reason = "Functional fixtures and assertions")]
use ruphish_contracts::{
    resources::request::{Group, Recipient},
    validation::{Code, Issue, ValidationReport},
};
use ruphish_ui::{
    form_state::FormState,
    recipient_editor::{RowDraft, Submission, initially_ordered},
};

fn recipient(email: &str) -> Recipient {
    Recipient {
        email: email.into(),
        ..Default::default()
    }
}

#[test]
fn initial_order_handles_empty_unicode_and_equal_emails_without_sorting_edits() {
    let values = [
        "z@example.test",
        "a@example.test",
        "",
        "a@example.test",
        "ä@example.test",
    ];
    let mut rows = initially_ordered(
        values
            .iter()
            .enumerate()
            .map(|(i, email)| {
                let value = Recipient {
                    first_name: i.to_string(),
                    ..recipient(email)
                };
                RowDraft::new(value.clone(), Some(value))
            })
            .collect(),
    );
    assert_eq!(
        rows.iter()
            .map(|r| r.value.first_name.as_str())
            .collect::<Vec<_>>(),
        ["2", "1", "3", "0", "4"]
    );
    assert!(rows.iter().all(|r| !r.dirty()));
    rows[0].value.email = "zz@example.test".into();
    assert_eq!(rows[0].value.first_name, "2");
    assert!(rows[0].dirty());
    rows[0].value.email.clear();
    assert!(!rows[0].dirty());
    assert!(RowDraft::new(Recipient::default(), None).dirty());
}

#[test]
fn submission_serializes_display_order_without_ids_and_maps_only_its_own_indices() {
    let submission = Submission {
        value: Group {
            id: 7,
            name: "Audience".into(),
            targets: vec![recipient("d@example.test"), recipient("a@example.test")],
        },
        rows: vec![42, 3],
    };
    assert_eq!(
        serde_json::to_value(&submission.value).unwrap(),
        serde_json::json!({
            "id":7,"name":"Audience","targets":[
                {"email":"d@example.test","first_name":"","last_name":"","position":""},
                {"email":"a@example.test","first_name":"","last_name":"","position":""}
            ]
        })
    );
    let mut report = ValidationReport {
        issues: [
            "targets.0.email",
            "targets.1.position",
            "targets",
            "name",
            "targets.99.email",
        ]
        .into_iter()
        .map(|field| Issue {
            field: field.into(),
            code: Code::Invalid,
        })
        .collect(),
    };
    submission.map_validation(&mut report);
    assert_eq!(
        report
            .issues
            .iter()
            .map(|issue| issue.field.as_str())
            .collect::<Vec<_>>(),
        [
            "targets.row-42.email",
            "targets.row-3.position",
            "targets",
            "name",
            "targets.99.email"
        ]
    );
    let mut validation = FormState::default();
    let ticket = validation.ticket();
    validation.edit("targets.row-42.email");
    assert!(
        !validation.receive(ticket, report.clone()),
        "late errors cannot annotate newer input"
    );
    assert!(validation.receive(validation.ticket(), report));
    validation.edit("targets");
    assert_eq!(
        validation
            .issues(&[])
            .iter()
            .map(|issue| issue.field.as_str())
            .collect::<Vec<_>>(),
        ["name"]
    );
    let before_reset = validation.ticket();
    validation.reset();
    assert!(validation.issues(&[]).is_empty());
    assert!(!validation.receive(before_reset, ValidationReport::default()));
}

#[test]
fn confirmed_save_preserves_newer_row_edits_and_advances_the_checkpoint() {
    let original = recipient("a@example.test");
    let mut row = RowDraft::new(original.clone(), Some(original));
    row.value.email = "b@example.test".into();
    let submitted = row.value.clone();
    row.value.first_name = "Newer edit".into();
    row.saved(&submitted, submitted.clone());
    assert_eq!(row.value.first_name, "Newer edit");
    assert!(row.dirty());
    row.value.first_name.clear();
    assert!(!row.dirty());
    row.value.email = "c@example.test".into();
    let submitted = row.value.clone();
    row.saved(&submitted, submitted.clone());
    assert_eq!(row.value, submitted);
    assert!(!row.dirty());
}
