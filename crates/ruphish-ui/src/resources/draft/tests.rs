use crate::resources::draft::*;
use ruphish_contracts::attribution::UserReference;
use ruphish_contracts::resources::details::Content;
use ruphish_contracts::resources::details::Group;
use ruphish_contracts::resources::details::Metadata;
use ruphish_contracts::resources::details::Profile;
use ruphish_contracts::resources::details::Resource;
use serde_json::json;

fn confirmed(id: i64, name: &str, revision: &str, content: Content) -> Confirmed {
    Confirmed::new(
        Resource {
            metadata: Metadata {
                id,
                name: name.into(),
                revision: 1,
                modified_date: Some(revision.into()),
                attribution: Default::default(),
            },
            content,
        },
        Some(revision.into()),
    )
    .unwrap()
}
fn profile(has_password: bool) -> Content {
    Content::Profile(Profile {
        interface_type: "SMTP".into(),
        host: "relay.example.test".into(),
        from_address: "sender@example.test".into(),
        username: None,
        has_password,
        ignore_cert_errors: false,
        headers: vec![],
    })
}
#[test]
fn confirmed_password_is_discarded_even_when_another_field_has_newer_edits() {
    let mut draft = ResourceDraft::loaded(confirmed(8, "Relay", "v1", profile(true)));
    draft.set_profile_password("replacement".into());
    let submitted = draft.value().clone();
    draft.edit(|value| value["name"] = json!("Newer name"));
    draft.saved(&submitted, confirmed(8, "Relay", "v2", profile(true)));
    assert!(draft.value().get("password").is_none());
    assert_eq!(draft.value()["name"], "Newer name");
    draft.cancel();
    assert_eq!(draft.value()["name"], "Relay");
    assert!(draft.value().get("password").is_none());
}
#[test]
fn keep_clear_and_a_newer_password_remain_distinct_during_save_and_import() {
    let mut draft = ResourceDraft::loaded(confirmed(8, "Relay", "v1", profile(true)));
    draft.set_profile_password(String::new());
    assert!(draft.value().get("password").is_none());
    draft.clear_profile_password(true);
    assert_eq!(draft.value()["password"], "");
    let submitted = draft.value().clone();
    draft.set_profile_password("newer replacement".into());
    draft.saved(&submitted, confirmed(8, "Relay", "v2", profile(false)));
    assert_eq!(draft.value()["password"], "newer replacement");
    assert_eq!(draft.value()["has_password"], false);
    draft
        .merge_import(
            &json!({"id":99,"has_password":true,"host":"other.example.test"}),
            "smtp",
        )
        .unwrap();
    assert_eq!(draft.value()["id"], 8);
    assert_eq!(draft.value()["host"], "other.example.test");
    assert_eq!(draft.value()["has_password"], false);
    draft.cancel();
    assert!(draft.value().get("password").is_none());
    assert_eq!(draft.value()["has_password"], false);
}
#[test]
fn late_save_preserves_newer_edits_and_cancel_uses_the_server_checkpoint() {
    let mut d = ResourceDraft::loaded(confirmed(
        1,
        "A",
        "old",
        Content::Group(Group { targets: vec![] }),
    ));
    d.edit(|v| v["name"] = json!("Submitted"));
    let submitted = d.value().clone();
    d.edit(|v| v["name"] = json!("Newer edit"));
    let mut saved = confirmed(
        1,
        "Submitted",
        "new",
        Content::Group(Group { targets: vec![] }),
    );
    saved.record.metadata.attribution.edited_by = Some(UserReference {
        id: 2,
        username: Some("bob".into()),
        deleted: false,
    });
    d.saved(&submitted, saved);
    assert_eq!(d.value()["name"], "Newer edit");
    assert!(d.dirty());
    assert_eq!(d.revision(), Some("new"));
    assert_eq!(
        d.confirmed()
            .unwrap()
            .metadata
            .attribution
            .edited_by
            .as_ref()
            .unwrap()
            .username
            .as_deref(),
        Some("bob")
    );
    d.merge_import(
        &json!({"created_at":"forged","created_by":999,"edited_by":999,"edited_at":"forged"}),
        "groups",
    )
    .unwrap();
    for field in [
        "created_at",
        "created_by",
        "edited_by",
        "edited_at",
        "revision",
        "modified_date",
    ] {
        assert!(
            d.value().get(field).is_none(),
            "server metadata cannot enter writable fields"
        );
    }
    assert_eq!(
        d.confirmed()
            .unwrap()
            .metadata
            .attribution
            .edited_by
            .as_ref()
            .unwrap()
            .id,
        2
    );
    d.cancel();
    assert_eq!(d.value()["name"], "Submitted");
    assert!(!d.dirty());
}
#[test]
fn server_normalization_becomes_the_checkpoint_after_success() {
    let value = json!({"id":0,"name":"  Example  ","targets":[]});
    let mut d = ResourceDraft::new(value.clone());
    d.saved(
        &value,
        confirmed(
            2,
            "Example",
            "v1",
            Content::Group(Group { targets: vec![] }),
        ),
    );
    assert_eq!(d.value()["name"], "Example");
    assert_eq!(d.value()["id"], 2);
    assert!(!d.dirty());
}
