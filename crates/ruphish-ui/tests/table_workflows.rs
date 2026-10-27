//! Ordering through editable drafts, bookmarks, pagination and typed table values.
#![allow(clippy::unwrap_used, reason = "Functional fixtures and assertions")]
use ruphish_contracts::reporting::Filter;
use ruphish_domain::reporting::{RecipientSort, SortDirection};
use ruphish_ui::{
    analysis_state,
    resource_state::ResourceDraft,
    table_state::{Sort, Value, ratio, scalar, timestamp},
};
use serde_json::json;

#[test]
fn sorted_recipient_edits_and_removal_keep_original_targets_and_saved_revision() {
    let original = json!({"id":7,"name":"Audience","targets":[
        {"email":"z@example.test","first_name":"Same","last_name":"","position":""},
        {"email":"a@example.test","first_name":"Same","last_name":"","position":""},
        {"email":"b@example.test","first_name":"","last_name":"","position":""}
    ]});
    let mut draft = ResourceDraft::new(original.clone(), Some("revision-1".into()));
    let sort = Sort {
        column: "email".into(),
        order: SortDirection::Asc,
    };
    let rows = draft.value()["targets"].as_array().unwrap();
    let displayed = sort.rows(
        (0..rows.len()).collect(),
        |index, key| scalar(&rows[*index][key]),
        |index| (*index).into(),
    );
    assert_eq!(displayed, [1, 2, 0]);
    assert!(!draft.dirty(), "sorting must not mutate the audience");
    draft.edit(|value| value["targets"][displayed[0]]["position"] = json!("Edited recipient"));
    assert_eq!(draft.value()["targets"][1]["email"], "a@example.test");
    assert_eq!(draft.value()["targets"][0]["position"], "");
    draft.edit(|value| {
        value["targets"]
            .as_array_mut()
            .unwrap()
            .remove(displayed[1]);
    });
    let submitted = draft.value().clone();
    let encoded = serde_json::to_vec(&submitted).unwrap();
    let decoded: ruphish_contracts::legacy::Group = serde_json::from_slice(&encoded).unwrap();
    let targets = decoded.targets;
    assert_eq!(targets.len(), 2);
    assert_eq!(targets[1].email, "a@example.test");
    draft.saved(&submitted, submitted.clone(), Some("revision-2".into()));
    assert!(!draft.dirty());
    assert_eq!(draft.revision(), Some("revision-2"));
    draft.edit(|value| value["targets"][0]["email"] = json!("pending@example.test"));
    draft.cancel();
    assert_eq!(draft.value(), &submitted);
}

#[test]
fn table_sort_and_page_bookmarks_survive_refresh_and_download_without_page_truncation() {
    let filter = Filter {
        sort: RecipientSort::Opened,
        order: SortDirection::Desc,
        offset: 50,
        limit: 25,
        recipients: "9,12".into(),
        source: "public".into(),
        from: Some(100),
        until: Some(200),
        ..Default::default()
    };
    let saved = analysis_state::query(&filter, false);
    let revisited: Filter = serde_urlencoded::from_str(&saved).unwrap();
    assert_eq!(revisited, filter);
    let mut download = revisited.clone();
    download.format = "csv".into();
    download.scope = "recipients".into();
    let exported: Filter =
        serde_urlencoded::from_str(&analysis_state::query(&download, true)).unwrap();
    assert_eq!(exported.sort, RecipientSort::Opened);
    assert_eq!(exported.order, SortDirection::Desc);
    assert_eq!(exported.offset, 0);
    assert_eq!(exported.recipients, "9,12");
    assert_eq!((exported.from, exported.until), (Some(100), Some(200)));
    let mut refreshed = vec![(12, 2), (9, 2), (7, 10)];
    let state = Sort {
        column: "count".into(),
        order: SortDirection::Desc,
    };
    assert_eq!(
        state.rows(
            refreshed.clone(),
            |r, _| Value::Number(r.1),
            |r| Value::Number(r.0)
        ),
        [(7, 10), (9, 2), (12, 2)]
    );
    refreshed[0].1 = 11;
    assert_eq!(
        state.rows(refreshed, |r, _| Value::Number(r.1), |r| Value::Number(r.0)),
        [(12, 11), (7, 10), (9, 2)]
    );
    assert_eq!(state.aria("count"), "descending");
    assert_eq!(state.aria("name"), "none");
    assert_eq!(state.select("count").aria("count"), "ascending");
}

#[test]
fn histories_and_rates_sort_actual_values_with_absence_unicode_and_stable_ties() {
    let rows = vec![
        (3, "2026-10-27T10:00:00+02:00"),
        (2, "2026-10-27T08:00:00Z"),
        (1, ""),
    ];
    let sort = Sort {
        column: "time".into(),
        order: SortDirection::Asc,
    };
    assert_eq!(
        sort.rows(rows, |r, _| timestamp(r.1), |r| Value::Number(r.0))
            .iter()
            .map(|r| r.0)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    let names = vec![
        (5, "\u{1f600}"),
        (4, "\u{e000}"),
        (3, "Same"),
        (2, "Same"),
        (1, ""),
    ];
    assert_eq!(
        sort.rows(names, |r, _| r.1.into(), |r| Value::Number(r.0))
            .iter()
            .map(|r| r.0)
            .collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
    let rates = vec![(4, 1, 100000), (3, 1, 99999), (2, 0, 1), (1, 0, 0)];
    assert_eq!(
        sort.rows(rates, |r, _| ratio(r.1, r.2), |r| Value::Number(r.0))
            .iter()
            .map(|r| r.0)
            .collect::<Vec<_>>(),
        [1, 2, 4, 3]
    );
}

#[test]
fn recent_campaign_window_stays_latest_three_when_sorting_and_refreshing() {
    use ruphish_contracts::overview::{Posture, RecentCampaign};
    use ruphish_ui::overview_state::recent_campaigns;
    let row = |id, name: &str, count| RecentCampaign {
        id,
        name: name.into(),
        status: "Completed".into(),
        launch: None,
        recipients: count,
        outcomes: Posture {
            delivered: 100,
            clicked: count,
            reported: count,
            ..Default::default()
        },
    };
    // Older rows would win either extreme if sorting were applied before the cutoff.
    let mut rows = vec![
        row(5, "Zulu", 30),
        row(4, "Mike", 20),
        row(3, "Bravo", 10),
        row(2, "Alpha", 0),
        row(1, "zzzz", 100),
    ];
    for column in ["name", "recipients", "clicked", "reported"] {
        for (order, expected) in [
            (SortDirection::Asc, [3, 4, 5]),
            (SortDirection::Desc, [5, 4, 3]),
        ] {
            let sort = Sort {
                column: column.into(),
                order,
            };
            let displayed = recent_campaigns(&rows, &sort);
            assert_eq!(displayed.iter().map(|r| r.id).collect::<Vec<_>>(), expected);
        }
    }
    rows.insert(0, row(6, "Echo", 15));
    let sort = Sort {
        column: "name".into(),
        order: SortDirection::Asc,
    };
    assert_eq!(
        recent_campaigns(&rows, &sort)
            .iter()
            .map(|r| r.id)
            .collect::<Vec<_>>(),
        [6, 4, 5]
    );
    assert_eq!(recent_campaigns(&rows[..2], &sort).len(), 2);
    assert!(recent_campaigns(&[], &sort).is_empty());
}
