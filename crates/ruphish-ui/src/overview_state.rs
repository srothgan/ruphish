//! Dashboard rates and recent campaign ordering, independent of query ownership.

/// Select the latest three campaigns before applying display ordering.
/// The API supplies recent rows newest first; sorting must never change this window.
pub fn recent_campaigns(
    rows: &[ruphish_contracts::overview::RecentCampaign],
    sort: &crate::table_state::Sort,
) -> Vec<ruphish_contracts::overview::RecentCampaign> {
    sort.rows(
        rows.iter().take(3).cloned().collect(),
        |row, key| match key {
            "recipients" => row.recipients.into(),
            "clicked" => crate::table_state::ratio(row.outcomes.clicked, row.outcomes.delivered),
            "reported" => crate::table_state::ratio(row.outcomes.reported, row.outcomes.delivered),
            _ => row.name.clone().into(),
        },
        |row| row.id.into(),
    )
}

/// Undefined rates stay visibly distinct from a measured zero.
pub fn percentage(count: i64, sent: i64) -> String {
    if sent <= 0 {
        "\u{2014}".into()
    } else {
        format!("{:.1}%", count as f64 / sent as f64 * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_or_null_counts_are_errors_instead_of_a_fake_empty_dashboard() {
        use ruphish_contracts::overview::PostureResponse;
        let valid = serde_json::json!({"window":"all", "since":null, "as_of":123,
            "delivered":0,"opened":0,"clicked":0,"reported":0,"credentials_entered":0,"previous":null});
        assert!(serde_json::from_value::<PostureResponse>(valid.clone()).is_ok());
        for field in [
            "delivered",
            "opened",
            "clicked",
            "reported",
            "credentials_entered",
        ] {
            let mut missing = valid.clone();
            if let Some(object) = missing.as_object_mut() {
                object.remove(field);
            }
            assert!(serde_json::from_value::<PostureResponse>(missing).is_err());
            let mut null = valid.clone();
            null[field] = serde_json::Value::Null;
            assert!(serde_json::from_value::<PostureResponse>(null).is_err());
        }
    }
    #[test]
    fn nullable_dashboard_fields_must_be_present() {
        use ruphish_contracts::overview::{PostureResponse, Snapshot};
        let complete = serde_json::json!({"window":"all","since":null,"as_of":123,"delivered":0,"opened":0,"clicked":0,"reported":0,"credentials_entered":0,"previous":null});
        for field in ["since", "previous"] {
            let mut value = complete.clone();
            value.as_object_mut().map(|v| v.remove(field));
            assert!(serde_json::from_value::<PostureResponse>(value).is_err());
        }
        let mut snapshot = serde_json::to_value(Snapshot::default()).expect("fixture");
        assert!(serde_json::from_value::<Snapshot>(snapshot.clone()).is_ok());
        snapshot["attention"]
            .as_object_mut()
            .map(|v| v.remove("next_launch"));
        assert!(serde_json::from_value::<Snapshot>(snapshot).is_err());
        let mut recent = serde_json::json!({"id":1,"name":"Historical","status":"Completed","launch":null,"recipients":0,"outcomes":{"delivered":0,"opened":0,"clicked":0,"reported":0,"credentials_entered":0}});
        assert!(
            serde_json::from_value::<ruphish_contracts::overview::RecentCampaign>(recent.clone())
                .unwrap()
                .launch
                .is_none()
        );
        recent.as_object_mut().unwrap().remove("launch");
        assert!(
            serde_json::from_value::<ruphish_contracts::overview::RecentCampaign>(recent).is_err()
        );
    }
    #[test]
    fn rate_distinguishes_no_sends_from_zero_outcomes() {
        assert_eq!(percentage(0, 0), "\u{2014}");
        assert_eq!(percentage(0, 4), "0.0%");
        assert_eq!(percentage(1, 3), "33.3%");
        assert_eq!(percentage(4, 4), "100.0%");
    }
}
