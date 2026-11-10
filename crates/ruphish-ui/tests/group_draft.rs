//! Reactive editing guarantees against the same owner used by the mounted group editor.
#![cfg(target_arch = "wasm32")]
#![allow(clippy::unwrap_used, reason = "Functional fixtures and assertions")]
use leptos::prelude::*;
use ruphish_contracts::{
    resource_details::Kind,
    resources::request::{Group, Recipient},
};
use ruphish_ui::{group_draft::GroupDraft, resource_state::ResourceDraft};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen_test::wasm_bindgen_test;
#[path = "support/resource_checkpoint.rs"]
mod resource_checkpoint;

fn loaded(count: usize) -> ResourceDraft {
    let value = Group {
        id: 7,
        name: "Audience".into(),
        targets: (0..count)
            .rev()
            .map(|i| Recipient {
                email: format!("{i:04}@example.test"),
                ..Default::default()
            })
            .collect(),
    };
    ResourceDraft::loaded(resource_checkpoint::checkpoint(
        Kind::Group,
        serde_json::to_value(value).unwrap(),
        "revision-1",
    ))
}

#[wasm_bindgen_test]
async fn typing_in_a_large_group_does_not_wake_other_rows_or_the_collection() {
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    let list_updates = Rc::new(Cell::new(0));
    let untouched_updates = Rc::new(Cell::new(0));
    let (group, edited) = owner.with(|| {
        let group = GroupDraft::new();
        group.load(&loaded(1000)).unwrap();
        let list_updates = list_updates.clone();
        Effect::new(move |_| {
            group.rows.track();
            list_updates.set(list_updates.get() + 1);
        });
        let rows = group.rows.get_untracked();
        for row in &rows[1..] {
            let value = row.draft.clone();
            let updates = untouched_updates.clone();
            Effect::new(move |_| {
                value.track();
                updates.set(updates.get() + 1);
            });
        }
        (group, rows[0].draft.clone())
    });
    any_spawner::Executor::tick().await;
    let before = (list_updates.get(), untouched_updates.get());
    assert_eq!(before, (1, 999));
    for email in ["D", "d@", "d@example.test"] {
        group.edit(&edited, |recipient| recipient.email = email.into());
        any_spawner::Executor::tick().await;
        assert_eq!((list_updates.get(), untouched_updates.get()), before);
        assert_eq!(group.submission(7).value.targets[0].email, email);
    }
    assert!(group.dirty());
    group.edit(&edited, |recipient| {
        recipient.email = "0000@example.test".into()
    });
    assert!(!group.dirty());
    group.prepend();
    let added = group.rows.with_untracked(|rows| rows[0].id);
    assert!(group.dirty());
    group.remove(added);
    assert!(
        !group.dirty(),
        "adding then removing a blank restores the checkpoint"
    );
    owner.cleanup();
}

#[wasm_bindgen_test]
fn save_delete_and_reload_keep_identity_and_newer_edits_coherent() {
    let owner = Owner::new();
    owner.with(|| {
        let group = GroupDraft::new();
        group.load(&loaded(3)).unwrap();
        let initial = group.rows.get_untracked();
        assert_eq!(
            group
                .submission(7)
                .value
                .targets
                .iter()
                .map(|row| row.email.as_str())
                .collect::<Vec<_>>(),
            [
                "0000@example.test",
                "0001@example.test",
                "0002@example.test"
            ]
        );
        group.prepend();
        let first = group.rows.get_untracked()[0].clone();
        group.edit(&first.draft, |row| row.email = "z@example.test".into());
        group.prepend();
        let second = group.rows.get_untracked()[0].clone();
        group.edit(&second.draft, |row| row.email = "a@example.test".into());
        group.remove(initial[1].id);
        group.remove(initial[1].id);
        let submitted = group.submission(7);
        assert_eq!(
            submitted.rows,
            [second.id, first.id, initial[0].id, initial[2].id]
        );
        assert_eq!(
            submitted
                .value
                .targets
                .iter()
                .map(|row| row.email.as_str())
                .collect::<Vec<_>>(),
            [
                "a@example.test",
                "z@example.test",
                "0000@example.test",
                "0002@example.test"
            ]
        );
        group.edit(&first.draft, |row| row.position = "During save".into());
        group.name.set("Newer name".into());
        group.remove(initial[0].id);
        group.saved(&submitted, submitted.value.clone());
        assert!(group.dirty());
        assert_eq!(
            first.draft.with_untracked(|row| row.value.position.clone()),
            "During save"
        );
        assert_eq!(group.name.get_untracked(), "Newer name");
        assert_eq!(
            group.submission(7).rows,
            [second.id, first.id, initial[2].id]
        );
        let retry = group.submission(7);
        group.saved(&retry, retry.value.clone());
        assert!(!group.dirty());
        assert_eq!(group.submission(7).rows, retry.rows);
        group.load(&loaded(1)).unwrap();
        assert!(!group.dirty());
        assert!(
            group
                .submission(7)
                .rows
                .iter()
                .all(|id| !submitted.rows.contains(id))
        );
    });
    owner.cleanup();
}
