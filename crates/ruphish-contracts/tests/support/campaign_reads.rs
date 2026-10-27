use super::*;
use serde_json::{Value, json};

fn snapshots() -> Value {
    serde_json::from_str(include_str!("../campaign-read-snapshots.json")).unwrap()
}

#[test]
fn incomplete_campaign_snapshots_are_errors_instead_of_empty_lists_or_zero_metrics() {
    let fixture = snapshots();
    assert_eq!(
        serde_json::from_value::<Directory>(fixture["directory"].clone())
            .unwrap()
            .items
            .len(),
        4
    );
    assert_eq!(
        serde_json::from_value::<Analysis>(fixture["analysis"].clone())
            .unwrap()
            .progression
            .values(),
        [1, 1, 1, 1, 0]
    );
    for kind in ["directory", "delivery", "analysis"] {
        for key in fixture[kind].as_object().unwrap().keys() {
            let mut broken = fixture[kind].clone();
            broken.as_object_mut().unwrap().remove(key);
            let rejected = match kind {
                "directory" => serde_json::from_value::<Directory>(broken).is_err(),
                "delivery" => serde_json::from_value::<Delivery>(broken).is_err(),
                _ => serde_json::from_value::<Analysis>(broken).is_err(),
            };
            assert!(rejected, "{kind}.{key}");
        }
    }
    let mut broken = fixture["analysis"].clone();
    broken["progression"] = json!([1, 1]);
    assert!(serde_json::from_value::<Analysis>(broken).is_err());
    let mut broken = fixture["analysis"].clone();
    broken["recipients"][0]
        .as_object_mut()
        .unwrap()
        .remove("counts");
    assert!(serde_json::from_value::<Analysis>(broken).is_err());
}

#[test]
fn sparse_counts_and_explicit_empty_pages_remain_valid() {
    let fixture = snapshots();
    let delivery: Delivery = serde_json::from_value(fixture["delivery"].clone()).unwrap();
    assert_eq!(delivery.counts.get("accepted"), Some(&1));
    assert!(!delivery.counts.contains_key("failed"));
    let empty: Directory = serde_json::from_value(json!({"items":[],"next_after":null})).unwrap();
    assert!(empty.items.is_empty());
    assert!(empty.next_after.is_none());
    let mut value = fixture["analysis"].clone();
    value["progression"] = json!({"accepted":0,"opened":0,"clicked":0,"submitted":0,"reported":0});
    assert_eq!(
        serde_json::from_value::<Analysis>(value)
            .unwrap()
            .progression
            .values(),
        [0; 5]
    );
}

#[test]
fn draft_null_selections_are_distinct_from_missing_read_fields() {
    let value = json!({"id":"draft-fixture-001", "revision":1,"step":0,"created":null,"modified":null,"campaign_id":null,"status":"Draft", "configuration":{"name":"","url":"","selection":{"groups":[],"template":{"id":0},"page":{"id":0},"smtp":{"id":0}},"policy":null,"launch_local":null,"send_by_local":null}});
    assert!(serde_json::from_value::<Draft>(value.clone()).is_ok());
    for key in [
        "name",
        "url",
        "selection",
        "policy",
        "launch_local",
        "send_by_local",
    ] {
        let mut broken = value.clone();
        broken["configuration"].as_object_mut().unwrap().remove(key);
        assert!(serde_json::from_value::<Draft>(broken).is_err(), "{key}");
    }
}

#[test]
fn named_metrics_and_classified_observations_reject_missing_or_unknown_values() {
    let fixture = snapshots()["analysis"].clone();
    for field in ["accepted", "opened", "clicked", "submitted", "reported"] {
        let mut broken = fixture.clone();
        broken["totals"].as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<Analysis>(broken).is_err(),
            "missing {field}"
        );
    }
    for pointer in [
        "/status",
        "/recipients/0/delivery",
        "/events/0/source",
        "/events/0/kind/value",
    ] {
        let mut broken = fixture.clone();
        *broken.pointer_mut(pointer).unwrap() = json!("unrecognized");
        assert!(
            serde_json::from_value::<Analysis>(broken).is_err(),
            "{pointer}"
        );
    }
    let mut imported = fixture;
    imported["events"][0]["kind"] = json!({"type":"historical","value":"Proxied request"});
    let value: Analysis = serde_json::from_value(imported).unwrap();
    assert_eq!(
        value.events[0].kind,
        ruphish_domain::events::RecordedKind::Historical("Proxied request".into())
    );
}
