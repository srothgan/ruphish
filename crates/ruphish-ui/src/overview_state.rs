//! Dashboard rates and comparisons, independent of query ownership.

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

/// Desired direction of an outcome; send volume has no inherent good direction.
#[derive(Clone, Copy)]
pub enum ChangeGoal {
    /// Operational volume, without a positive/negative interpretation.
    Neutral,
    /// More outcomes are desirable, such as reports.
    Increase,
    /// Fewer outcomes are desirable, such as clicks.
    Decrease,
}
impl ChangeGoal {
    /// Semantic styling shared by the count and its accessible explanation.
    pub fn tone(self, current: i64, previous: i64) -> &'static str {
        if current == previous || matches!(self, Self::Neutral) {
            "neutral"
        } else if (current > previous) == matches!(self, Self::Increase) {
            "improving"
        } else {
            "worsening"
        }
    }
}

/// Absolute recipient change; avoids undefined percentage growth from zero.
pub fn count_change(current: i64, previous: i64) -> String {
    match current.cmp(&previous) {
        std::cmp::Ordering::Greater => format!("\u{2191} +{}", current - previous),
        std::cmp::Ordering::Less => format!("\u{2193} -{}", previous - current),
        std::cmp::Ordering::Equal => "No change".into(),
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
    fn outcome_trends_distinguish_reporting_from_risky_interactions_and_send_volume() {
        for (current, previous) in [(3, 0), (5, 2)] {
            assert_eq!(ChangeGoal::Increase.tone(current, previous), "improving");
            assert_eq!(ChangeGoal::Decrease.tone(current, previous), "worsening");
            assert_eq!(ChangeGoal::Increase.tone(previous, current), "worsening");
            assert_eq!(ChangeGoal::Decrease.tone(previous, current), "improving");
            assert_eq!(ChangeGoal::Neutral.tone(current, previous), "neutral");
            assert_eq!(ChangeGoal::Neutral.tone(previous, current), "neutral");
        }
        for goal in [
            ChangeGoal::Neutral,
            ChangeGoal::Increase,
            ChangeGoal::Decrease,
        ] {
            assert_eq!(goal.tone(0, 0), "neutral");
            assert_eq!(goal.tone(4, 4), "neutral");
        }
    }
    #[test]
    fn count_changes_handle_zero_baselines_decreases_and_equal_periods() {
        assert_eq!(count_change(3, 0), "\u{2191} +3");
        assert_eq!(count_change(0, 4), "\u{2193} -4");
        assert_eq!(count_change(5, 2), "\u{2191} +3");
        assert_eq!(count_change(2, 5), "\u{2193} -3");
        assert_eq!(count_change(0, 0), "No change");
        assert_eq!(count_change(4, 4), "No change");
    }
    #[test]
    fn rate_distinguishes_no_sends_from_zero_outcomes() {
        assert_eq!(percentage(0, 0), "\u{2014}");
        assert_eq!(percentage(0, 4), "0.0%");
        assert_eq!(percentage(1, 3), "33.3%");
        assert_eq!(percentage(4, 4), "100.0%");
    }
}
