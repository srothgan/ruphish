//! Dashboard reads: keep confirmed data on refresh failure, never mix periods.

/// A request belongs to one selection and one generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request(u64);

/// Independent request state for either the dashboard snapshot or its metrics.
#[derive(Clone, Debug)]
pub struct ReadState<T> {
    key: String,
    generation: u64,
    /// Last confirmed response for the current selection.
    pub data: Option<T>,
    /// Current request is in flight.
    pub loading: bool,
    /// Most recent failure for the current selection.
    pub error: Option<String>,
}
impl<T> Default for ReadState<T> {
    fn default() -> Self {
        Self {
            key: String::new(),
            generation: 0,
            data: None,
            loading: false,
            error: None,
        }
    }
}
impl<T> ReadState<T> {
    /// Start a read; a different selection invalidates the previous data.
    pub fn begin(&mut self, key: &str) -> Request {
        if self.key != key {
            self.data = None;
            self.key = key.into();
        }
        self.generation += 1;
        self.loading = true;
        self.error = None;
        Request(self.generation)
    }
    /// Ignore late results, including failures, from an obsolete selection.
    pub fn finish(&mut self, request: Request, result: Result<T, String>) -> bool {
        if request.0 != self.generation {
            return false;
        }
        self.loading = false;
        match result {
            Ok(data) => {
                self.data = Some(data);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
        true
    }
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
    fn changing_period_ignores_old_success_and_failure_without_unlocking_new_request() {
        let mut state = ReadState::default();
        let old = state.begin("all");
        let next = state.begin("7d");
        assert!(!state.finish(old, Ok(900)));
        assert!(!state.finish(old, Err("timeout".into())));
        assert!(state.loading);
        assert_eq!(state.data, None);
        assert_eq!(state.error, None);
        assert!(state.finish(next, Ok(3)));
        assert_eq!(state.data, Some(3));
        assert!(!state.loading);
    }
    #[test]
    fn failed_refresh_keeps_confirmed_counts_and_allows_retry_but_new_period_does_not() {
        let mut state = ReadState::default();
        let first = state.begin("all");
        state.finish(first, Ok(12));
        let refresh = state.begin("all");
        state.finish(refresh, Err("offline".into()));
        assert_eq!(state.data, Some(12));
        assert!(!state.loading);
        assert_eq!(state.error.as_deref(), Some("offline"));
        let retry = state.begin("all");
        state.finish(retry, Ok(14));
        assert_eq!(state.data, Some(14));
        assert_eq!(state.error, None);
        let other = state.begin("30d");
        state.finish(other, Err("offline".into()));
        assert_eq!(state.data, None);
        assert!(!state.loading);
    }
    #[test]
    fn metrics_filter_does_not_reload_or_clear_snapshot() {
        let mut snapshot = ReadState::default();
        let mut metrics = ReadState::default();
        let request = snapshot.begin("snapshot");
        snapshot.finish(request, Ok(5));
        let request = metrics.begin("all");
        metrics.finish(request, Ok(25));
        metrics.begin("7d");
        assert_eq!(snapshot.data, Some(5));
        assert!(!snapshot.loading);
        assert_eq!(metrics.data, None);
        assert!(metrics.loading);
    }
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
