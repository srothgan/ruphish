//! Recipient expansion survives table refresh; history merges only confirmed responses.
use crate::api_error::ApiError;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Default)]
/// Confirmed history and the request currently owned by one expanded recipient.
pub struct History {
    /// User-controlled expansion, independent of table refresh.
    pub open: bool,
    /// Latest current request failure; confirmed records remain available.
    pub error: Option<String>,
    /// Attempts indexed by their persistent generation.
    pub attempts: BTreeMap<i64, Value>,
    /// Sanitized events indexed by their persistent ID.
    pub events: BTreeMap<i64, Value>,
    /// Exclusive cursor for older attempts, absent at the beginning of history.
    pub next_attempt: Option<i64>,
    /// Exclusive cursor for older events, absent at the beginning of history.
    pub next_event: Option<i64>,
    sequence: u64,
    active: Option<(u64, bool)>,
    initialized: bool,
    older_loaded: bool,
    /// Retry the failed direction without discarding loaded pages.
    pub retry_older: bool,
}
impl History {
    /// A confirmed write supersedes any history response begun before that write.
    pub fn invalidate_reads(&mut self) {
        self.active = None;
        self.sequence += 1;
    }

    /// Whether an initial history read has completed successfully.
    pub fn has_loaded(&self) -> bool {
        self.initialized
    }

    /// Whether this expansion currently owns an in-flight request.
    pub fn loading(&self) -> bool {
        self.active.is_some()
    }
    /// Closing invalidates pending responses; reopening permits fresh validation.
    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.error = None;
        }
        if !self.open {
            self.active = None;
        }
    }
    /// Start a bounded request, suppressing parallel loads for this recipient.
    pub fn begin(&mut self, older: bool) -> Option<(u64, String)> {
        if !self.open
            || self.loading()
            || (older && self.next_attempt.is_none() && self.next_event.is_none())
        {
            return None;
        }
        self.sequence += 1;
        self.active = Some((self.sequence, older));
        self.retry_older = older;
        self.error = None;
        let query = if older {
            format!(
                "?before_attempt={}&before_event={}",
                self.next_attempt.unwrap_or(0),
                self.next_event.unwrap_or(0)
            )
        } else {
            String::new()
        };
        Some((self.sequence, query))
    }
    /// Accept current responses before changing records, cursors or failure state.
    pub fn finish(&mut self, request: u64, result: Result<Value, ApiError>) {
        let Some((active, older)) = self.active else {
            return;
        };
        if active != request || !self.open {
            return;
        }
        self.active = None;
        let value = match result {
            Ok(v) => v,
            Err(e) => {
                if e.is_terminal() {
                    self.events.clear();
                    self.attempts.clear();
                    self.next_event = None;
                    self.next_attempt = None;
                    self.initialized = false;
                    self.older_loaded = false;
                    self.retry_older = false;
                }
                self.error = Some(e.message);
                return;
            }
        };
        // If a fresh page no longer overlaps loaded history, resume from its
        // boundary so a burst of new records cannot leave an invisible gap.
        let disjoint = |key: &str, id: &str, map: &BTreeMap<i64, Value>| {
            let oldest = value[key]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|r| r[id].as_i64())
                .min();
            oldest
                .zip(map.last_key_value().map(|(id, _)| *id))
                .is_some_and(|(oldest, newest)| oldest > newest)
        };
        let attempt_gap = !older && disjoint("attempts", "generation", &self.attempts);
        let event_gap = !older
            && (value["refresh_event_gap"] == true || disjoint("events", "id", &self.events));
        if !older {
            self.events.clear();
        }
        for (key, id, map) in [
            ("attempts", "generation", &mut self.attempts),
            ("events", "id", &mut self.events),
        ] {
            for row in value[key].as_array().into_iter().flatten() {
                if let Some(id) = row[id].as_i64() {
                    if key == "events" {
                        let revision = |v: &Value| v["record"]["revision"].as_i64().unwrap_or(0);
                        if map
                            .get(&id)
                            .is_none_or(|old| revision(row) >= revision(old))
                        {
                            map.insert(id, row.clone());
                        }
                    } else if older {
                        map.entry(id).or_insert_with(|| row.clone());
                    } else {
                        map.insert(id, row.clone());
                    }
                }
            }
        }
        if older || !self.initialized || !self.older_loaded {
            self.next_attempt = value["next_attempt"].as_i64();
            self.next_event = value["next_event"].as_i64();
        } else {
            if attempt_gap && value["next_attempt"].is_number() {
                self.next_attempt = value["next_attempt"].as_i64();
            }
            if event_gap && value["next_event"].is_number() {
                self.next_event = value["next_event"].as_i64();
            }
        }
        self.initialized = true;
        self.older_loaded |= older;
    }
}

/// Consistent semantic colors for state labels and timeline markers.
pub fn tone(state: &str) -> &'static str {
    match state {
        "accepted" => "success",
        "failed" | "blocked" => "failure",
        "unknown" | "retry" => "warning",
        _ => "neutral",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn obsolete_terminal_failures_cannot_erase_a_confirmed_edit_or_new_request() {
        for status in [401, 403, 404] {
            let mut history = History::default();
            history.toggle();
            let (initial, _) = history.begin(false).unwrap();
            history.finish(
                initial,
                Ok(json!({
                    "events":[{"id":5,"record":{"revision":0}}],
                    "attempts":[{"generation":2,"state":"accepted"}],
                    "next_event":5,"next_attempt":2
                })),
            );
            let (stale, _) = history.begin(false).unwrap();
            // Same mounted history: the event-edit acknowledgement invalidates reads.
            history.invalidate_reads();
            history
                .events
                .insert(5, json!({"id":5,"record":{"revision":1}}));
            let (current, _) = history.begin(true).unwrap();
            history.finish(
                stale,
                Err(ApiError::http(status, String::new(), "Obsolete".into())),
            );
            assert_eq!(history.events[&5]["record"]["revision"], 1);
            assert_eq!(history.attempts[&2]["state"], "accepted");
            assert_eq!(history.next_event, Some(5));
            assert_eq!(history.next_attempt, Some(2));
            assert!(history.loading());
            assert!(history.error.is_none());
            assert!(history.has_loaded());
            history.finish(
                current,
                Err(ApiError::http(503, String::new(), "Retry".into())),
            );
            assert_eq!(history.error.as_deref(), Some("Retry"));
            assert_eq!(history.events.len(), 1);
            assert_eq!(history.attempts.len(), 1);
            assert_eq!(history.next_event, Some(5));
            assert_eq!(history.next_attempt, Some(2));
            assert!(history.retry_older);
            let (current, _) = history.begin(true).unwrap();
            history.finish(
                current,
                Err(ApiError::http(status, String::new(), "Current".into())),
            );
            assert!(history.events.is_empty());
            assert!(history.attempts.is_empty());
            assert!(history.next_event.is_none());
            assert!(history.next_attempt.is_none());
            assert!(!history.has_loaded());
            assert!(!history.loading());
            assert!(!history.retry_older);
            assert_eq!(history.error.as_deref(), Some("Current"));
            assert!(history.open);
        }
    }

    #[test]
    fn confirmed_writes_invalidate_old_reads_and_refresh_removes_deleted_rows() {
        let mut history = History::default();
        history.toggle();
        let (r, _) = history.begin(false).unwrap();
        history.finish(
            r,
            Ok(json!({"events":[{"id":5,"record":{"revision":0}}],"next_event":5})),
        );
        let (r, _) = history.begin(true).unwrap();
        history.finish(
            r,
            Ok(json!({"events":[{"id":1,"record":{"revision":0}}],"next_event":null})),
        );
        let (stale, _) = history.begin(false).unwrap();
        history.invalidate_reads();
        history
            .events
            .insert(5, json!({"id":5,"record":{"revision":1,"edited":true}}));
        history.finish(
            stale,
            Ok(json!({"events":[{"id":5,"record":{"revision":0}}]})),
        );
        assert_eq!(history.events[&5]["record"]["revision"], 1);
        assert!(history.events.contains_key(&1));
        let (r, _) = history.begin(false).unwrap();
        history.finish(r, Ok(json!({"events":[{"id":5,"record":{"revision":1,"edited":true}}],"next_event":null})));
        assert!(!history.events.contains_key(&1));
        assert_eq!(history.events[&5]["record"]["edited"], true);
    }
    #[test]
    fn expansion_refresh_retry_and_older_pages_preserve_confirmed_history() {
        let mut h = History::default();
        assert!(h.begin(false).is_none());
        h.toggle();
        let (first, _) = h.begin(false).unwrap();
        assert!(h.begin(false).is_none());
        h.finish(first,Ok(json!({"attempts":[{"generation":2,"state":"submitting"}],"events":[{"id":102}],"next_event":102,"next_attempt":2})));
        let (old, query) = h.begin(true).unwrap();
        assert_eq!(query, "?before_attempt=2&before_event=102");
        h.finish(old, Err(ApiError::local("Connection failed".into())));
        assert!(h.open);
        assert_eq!(h.events.len(), 1);
        assert!(h.retry_older);
        let (retry, _) = h.begin(true).unwrap();
        h.finish(retry,Ok(json!({"attempts":[{"generation":1,"state":"retry"}],"events":[{"id":1}],"next_event":null,"next_attempt":null})));
        let (refresh, _) = h.begin(false).unwrap();
        h.finish(refresh,Ok(json!({"attempts":[{"generation":2,"state":"accepted"}],"events":[{"id":103},{"id":102},{"id":1}],"next_event":102,"next_attempt":2})));
        assert_eq!(h.attempts.len(), 2);
        assert_eq!(h.attempts[&2]["state"], "accepted");
        assert_eq!(h.events.len(), 3);
        assert!(h.next_event.is_none());
        let (stale, _) = h.begin(false).unwrap();
        h.toggle();
        h.toggle();
        let (current, _) = h.begin(false).unwrap();
        h.finish(stale, Err(ApiError::local("Stale failure".into())));
        assert!(h.loading());
        assert!(h.error.is_none());
        h.finish(current, Err(ApiError::local("Current failure".into())));
        assert!(!h.loading());
        assert!(h.error.is_some());
        assert_eq!(h.events.len(), 3);
    }
    #[test]
    fn repeated_events_keep_individual_identities_and_occurrence_times() {
        let mut h = History::default();
        h.toggle();
        let (request, _) = h.begin(false).unwrap();
        h.finish(request, Ok(json!({"events":[{"id":1,"at":32,"message":"Clicked Link"},{"id":2,"at":32,"message":"Clicked Link"}]})));
        assert_eq!(h.events.len(), 2);
        assert_eq!(h.events[&1]["at"], 32);
        assert_eq!(h.events[&2]["at"], 32);
    }

    #[test]
    fn a_refresh_burst_reopens_pagination_instead_of_hiding_a_history_gap() {
        let mut h = History::default();
        h.toggle();
        let (r, _) = h.begin(false).unwrap();
        h.finish(r, Ok(json!({"events":[{"id":2}],"next_event":2})));
        let (r, _) = h.begin(true).unwrap();
        h.finish(r, Ok(json!({"events":[{"id":1}],"next_event":null})));
        assert!(h.next_event.is_none());
        let (r, _) = h.begin(false).unwrap();
        h.finish(
            r,
            Ok(json!({"events":[{"id":105},{"id":104},{"id":2},{"id":1}],"refresh_event_gap":true,"next_event":104})),
        );
        assert_eq!(h.next_event, Some(104));
        let (r, q) = h.begin(true).unwrap();
        assert_eq!(q, "?before_attempt=0&before_event=104");
        h.finish(
            r,
            Ok(json!({"events":[{"id":103},{"id":2}],"next_event":2})),
        );
        assert!(h.events.contains_key(&1));
        assert!(h.events.contains_key(&103));
        assert_eq!(h.events.len(), 5);
    }
}
