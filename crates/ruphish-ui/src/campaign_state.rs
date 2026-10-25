//! Review and launch state prevents stale responses and accidental duplicate launches.
use ruphish_contracts::campaign_workflow::Review;
use serde_json::{Value, json};

#[derive(Clone, PartialEq, Eq)]
struct ReviewInputs {
    visible: bool,
    resources_loading: bool,
    can_edit: bool,
    retry: u64,
    generation: u64,
    policy_ready: bool,
    frozen: bool,
}

/// Automatic review lifecycle shared by the UI effect and functional scenarios.
#[derive(Default)]
pub struct ReviewFlow {
    previous: Option<ReviewInputs>,
}
impl ReviewFlow {
    /// Return an HTTP request snapshot only when review inputs/visit change.
    /// Finishing a request, displaying its error or editing the test recipient
    /// must not cause a request loop or discard an uncertain launch.
    pub fn update(
        &mut self,
        draft: &mut Draft,
        visible: bool,
        resources_loading: bool,
        can_edit: bool,
        retry: u64,
    ) -> Option<((u64, u64), Value)> {
        let inputs = ReviewInputs {
            visible,
            resources_loading,
            can_edit,
            retry,
            generation: draft.generation,
            policy_ready: draft.policy_ready(),
            frozen: draft.launch.is_some(),
        };
        if self.previous.as_ref() == Some(&inputs) {
            return None;
        }
        self.previous = Some(inputs.clone());
        if inputs.frozen {
            return None;
        }
        if !visible || resources_loading || !can_edit || !inputs.policy_ready {
            draft.clear_review();
            return None;
        }
        draft.begin_review()
    }
}

/// The editable request and its last authoritative review.
#[derive(Clone, Default)]
pub struct Draft {
    /// Current form values.
    pub value: Value,
    /// Server-confirmed selection and schedule.
    pub review: Option<Review>,
    /// Request identity and draft generation currently being validated.
    pub review_request: Option<(u64, u64)>,
    /// Validation/network failure for the current review only.
    pub review_error: Option<String>,
    /// Distinguishes retries and repeated visits without changing the draft.
    pub review_sequence: u64,
    /// Edit generation, used to discard a stale review response.
    pub generation: u64,
    /// Stable key and exact body retained while a launch response is uncertain.
    pub launch: Option<(String, Value)>,
    /// Selected page request identity; late responses must not replace newer defaults.
    pub page_request: Option<(i64, u64)>,
}
impl Draft {
    /// Select a page and replace the prior page's policy once its defaults arrive.
    pub fn select_page(&mut self, id: i64) -> Option<(i64, u64)> {
        if self.launch.is_some() {
            return None;
        }
        if self.value["policy"]["learning_page_id"].is_number() {
            self.edit(|v| v["selection"]["page"] = json!({"id":id}));
            self.page_request = None;
            return None;
        }
        self.edit(|v| {
            v["selection"]["page"] = json!({"id":id});
            v["policy"] = if id == 0 {
                json!({"redirect_url":""})
            } else {
                Value::Null
            };
        });
        self.page_request = (id != 0).then_some((id, self.generation));
        self.page_request
    }
    /// Apply only complete defaults from the currently requested page; failures block review.
    pub fn page_loaded(&mut self, request: (i64, u64), page: Option<&Value>) -> bool {
        if self.page_request != Some(request) || self.launch.is_some() {
            return false;
        }
        self.page_request = None;
        if let Some(page) = page.filter(|page| {
            page["id"].as_i64() == Some(request.0) && page["redirect_url"].is_string()
        }) {
            self.edit(|v| {
                v["policy"] = json!({
                    "redirect_url":page["redirect_url"]
                });
            });
        }
        true
    }
    /// The operator must see concrete policy values before requesting a review.
    pub fn policy_ready(&self) -> bool {
        self.page_request.is_none() && self.value["policy"]["redirect_url"].is_string()
    }
    /// Refuse edits while the outcome of a launch may be unknown.
    pub fn edit(&mut self, update: impl FnOnce(&mut Value)) {
        if self.launch.is_none() {
            update(&mut self.value);
            self.generation += 1;
            self.clear_review();
        }
    }
    /// Leaving review or changing inputs invalidates pending responses and old output.
    pub fn clear_review(&mut self) {
        if self.launch.is_none() {
            self.review = None;
            self.review_request = None;
            self.review_error = None;
        }
    }
    /// Start validation for the current visit; uncertain launches keep their frozen review.
    pub fn begin_review(&mut self) -> Option<((u64, u64), Value)> {
        if self.launch.is_some() {
            return None;
        }
        self.clear_review();
        self.review_sequence += 1;
        let request = (self.review_sequence, self.generation);
        self.review_request = Some(request);
        Some((request, self.value.clone()))
    }
    /// Only this visit's response can certify the unchanged draft or display an error.
    pub fn reviewed(&mut self, request: (u64, u64), result: Result<Review, String>) -> bool {
        if self.review_request != Some(request)
            || request.1 != self.generation
            || self.launch.is_some()
        {
            return false;
        }
        self.review_request = None;
        match result {
            Ok(review) => self.review = Some(review),
            Err(error) => self.review_error = Some(error),
        }
        true
    }
    /// Freeze reviewed revisions and reuse the same launch key/body on a retry.
    pub fn launch(&mut self, key: String) -> Option<(String, Value)> {
        if let Some(pending) = &self.launch {
            return Some(pending.clone());
        }
        let review = self.review.as_ref()?;
        let mut value = self.value.clone();
        value["selection"] = json!(review.selection);
        value["policy"] = json!(review.policy);
        self.launch = Some((key, value));
        self.launch.clone()
    }
    /// A definite rejection requires a fresh review; a transport error must not call this.
    pub fn rejected(&mut self) {
        self.launch = None;
        self.clear_review();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn review(patch: Value) -> Review {
        fn merge(base: &mut Value, patch: Value) {
            if let (Some(base), Some(fields)) = (base.as_object_mut(), patch.as_object()) {
                for (key, value) in fields {
                    merge(base.entry(key).or_insert(Value::Null), value.clone());
                }
            } else {
                *base = patch;
            }
        }
        let mut value = json!({"total":1,"duplicate_count":0,"launch_date":123,"send_by_date":null,
            "selection":{"groups":[],"template":{"id":1,"name":"Template","revision":"v1"},"page":{"id":2,"name":"Page","revision":"v1"},"smtp":{"id":3,"name":"SMTP","revision":"v1"}},
            "policy":{"capture_credentials":true,"capture_passwords":true,"redirect_url":""},"mailer_enabled":true,"conflict_count":0,"conflicts":[],
            "preview":{"subject":"Subject","html":"<p>Preview</p>","text":"Preview","visible_sender":"Sender","transport_sender":"Sender"}});
        merge(&mut value, patch);
        serde_json::from_value(value).unwrap()
    }
    fn campaign_draft() -> Draft {
        Draft {
            value: json!({"name":"Training","policy":{"redirect_url":""}}),
            ..Default::default()
        }
    }
    #[test]
    fn learning_destination_survives_landing_selection_review_and_uncertain_launch_retry() {
        let mut draft = Draft {
            value: json!({"selection":{"page":{"id":1}},"policy":{"redirect_url":"","learning_page_id":42}}),
            ..Default::default()
        };
        assert!(draft.select_page(2).is_none());
        assert_eq!(draft.value["policy"]["learning_page_id"], 42);
        let (request, _) = draft.begin_review().unwrap();
        assert!(draft.reviewed(
            request,
            Ok(review(json!({"policy":{"learning_page_id":42}})))
        ));
        let first = draft.launch("learning-launch".into()).unwrap();
        assert_eq!(first.1["policy"]["learning_page_id"], 42);
        draft.edit(|value| value["policy"]["learning_page_id"] = json!(99));
        assert_eq!(draft.launch("different-key".into()).unwrap(), first);
    }
    #[test]
    fn automatic_review_waits_for_resources_and_defaults_then_revalidates_each_visit() {
        let mut flow = ReviewFlow::default();
        let mut d = campaign_draft();
        assert!(flow.update(&mut d, false, false, true, 0).is_none());
        assert!(flow.update(&mut d, true, true, true, 0).is_none());
        let page = d.select_page(7).unwrap();
        assert!(flow.update(&mut d, true, false, true, 0).is_none());
        assert!(d.launch("premature".into()).is_none());
        d.page_loaded(page, Some(&json!({"id":7,"redirect_url":""})));
        let (first, sent) = flow.update(&mut d, true, false, true, 0).unwrap();
        assert_eq!(sent["selection"]["page"]["id"], 7);
        assert!(flow.update(&mut d, true, false, true, 0).is_none());
        d.reviewed(first, Ok(review(json!({"total":1}))));
        assert!(flow.update(&mut d, true, false, true, 0).is_none());
        assert_eq!(d.review.as_ref().unwrap().total, 1);
        assert!(flow.update(&mut d, false, false, true, 0).is_none());
        assert!(d.review.is_none());
        let (revisited, body) = flow.update(&mut d, true, false, true, 0).unwrap();
        assert_eq!(body, sent);
        assert_ne!(revisited, first);
        d.edit(|v| v["name"] = json!("Edited training"));
        let (edited, body) = flow.update(&mut d, true, false, true, 0).unwrap();
        assert_eq!(body["name"], "Edited training");
        assert!(!d.reviewed(revisited, Ok(review(json!({"total":100})))));
        assert!(!d.reviewed(revisited, Err("Old network error".into())));
        assert_eq!(d.review_request, Some(edited));
    }
    #[test]
    fn automatic_review_failure_waits_for_retry_or_edits_and_refresh_revokes_old_validation() {
        let mut flow = ReviewFlow::default();
        let mut d = campaign_draft();
        let (invalid, _) = flow.update(&mut d, true, false, true, 0).unwrap();
        d.reviewed(invalid, Err("Missing recipient group".into()));
        assert!(flow.update(&mut d, true, false, true, 0).is_none());
        assert!(d.launch("invalid".into()).is_none());
        let (retry, _) = flow.update(&mut d, true, false, true, 1).unwrap();
        assert!(d.review_error.is_none());
        d.reviewed(retry, Err("Network request failed".into()));
        assert!(flow.update(&mut d, true, false, true, 1).is_none());
        d.edit(|v| v["selection"]["groups"] = json!([{"id":3}]));
        let (corrected, body) = flow.update(&mut d, true, false, true, 1).unwrap();
        assert_eq!(body["selection"]["groups"], json!([{"id":3}]));
        d.reviewed(corrected, Ok(review(json!({"total":2}))));
        assert!(flow.update(&mut d, true, false, true, 1).is_none());
        assert!(flow.update(&mut d, true, true, true, 1).is_none());
        assert!(d.review.is_none());
        assert!(d.launch("refreshing".into()).is_none());
        let (refreshed, _) = flow.update(&mut d, true, false, true, 1).unwrap();
        assert!(!d.reviewed(corrected, Err("Late failure".into())));
        d.reviewed(refreshed, Ok(review(json!({"total":3}))));
        assert!(flow.update(&mut d, true, false, false, 1).is_none());
        assert!(
            d.review.is_none(),
            "Permission loss cannot leave a launchable review"
        );
    }
    #[test]
    fn automatic_review_never_replaces_an_uncertain_launch_but_revalidates_a_definite_rejection() {
        let mut flow = ReviewFlow::default();
        let mut d = campaign_draft();
        let (request, _) = flow.update(&mut d, true, false, true, 0).unwrap();
        d.reviewed(request, Ok(review(json!({"selection":{"template":{"id":7,"revision":"v1"}},"policy":{"capture_passwords":false}}))));
        let original = d.launch("original".into()).unwrap();
        assert!(flow.update(&mut d, true, false, true, 0).is_none());
        assert!(flow.update(&mut d, true, true, true, 1).is_none());
        assert!(flow.update(&mut d, true, false, true, 1).is_none());
        assert_eq!(d.launch("different".into()).unwrap(), original);
        d.rejected();
        let (fresh, _) = flow.update(&mut d, true, false, true, 2).unwrap();
        assert!(d.launch("not-ready".into()).is_none());
        assert!(!d.reviewed(request, Ok(review(json!({"total":999})))));
        d.reviewed(fresh, Ok(review(json!({"selection":{"template":{"id":7,"revision":"v2"}},"policy":{"capture_passwords":false}}))));
        let (key, body) = d.launch("new-attempt".into()).unwrap();
        assert_eq!(key, "new-attempt");
        assert_eq!(body["selection"]["template"]["revision"], "v2");
    }
    #[test]
    fn selected_defaults_are_visible_editable_and_explicit_in_the_launch() {
        let mut d = Draft {
            value: json!({}),
            ..Default::default()
        };
        let request = d.select_page(12).unwrap();
        assert!(!d.policy_ready());
        assert!(d.page_loaded(
            request,
            Some(&json!({"id":12,"redirect_url":"https://training.example.test/finish"}))
        ));
        assert_eq!(
            d.value["policy"],
            json!({"redirect_url":"https://training.example.test/finish"})
        );
        assert!(d.policy_ready());
        d.edit(|v| v["policy"]["redirect_url"] = json!(""));
        let explicit = json!({"redirect_url":""});
        assert_eq!(d.value["policy"], explicit);
        let (request, _) = d.begin_review().unwrap();
        assert!(d.reviewed(
            request,
            Ok(review(
                json!({"selection":{"page":{"id":12,"revision":"reviewed"}},"policy":explicit})
            ))
        ));
        let (_, body) = d.launch("stable-key".into()).unwrap();
        assert_eq!(
            body["policy"],
            json!({"capture_credentials":true,"capture_passwords":true,"redirect_url":""})
        );
        assert!(d.select_page(13).is_none());
        assert_eq!(d.launch("different-key".into()).unwrap().1, body);
    }
    #[test]
    fn page_changes_discard_stale_defaults_and_failures_without_losing_other_edits() {
        let mut d = Draft {
            value: json!({}),
            ..Default::default()
        };
        let old = d.select_page(1).unwrap();
        let current = d.select_page(2).unwrap();
        d.edit(|v| v["name"] = json!("Keep this campaign name"));
        assert!(!d.page_loaded(old, None));
        assert_eq!(d.page_request, Some(current));
        assert!(!d.page_loaded(old, Some(&json!({"id":1,"redirect_url":"old"}))));
        assert!(d.page_loaded(current, Some(&json!({"id":2,"redirect_url":""}))));
        assert_eq!(d.value["name"], "Keep this campaign name");
        assert_eq!(d.value["policy"], json!({"redirect_url":""}));
        let retry = d.select_page(2).unwrap();
        assert!(!d.page_loaded(current, None));
        assert!(d.page_loaded(retry, None));
        assert!(!d.policy_ready());
        for response in [json!({"id":2}), json!({"id":3,"redirect_url":"wrong page"})] {
            let retry = d.select_page(2).unwrap();
            assert!(d.page_loaded(retry, Some(&response)));
            assert!(!d.policy_ready());
        }
        let retry = d.select_page(2).unwrap();
        assert!(d.select_page(0).is_none());
        assert!(!d.page_loaded(retry, Some(&json!({"id":2,"redirect_url":"late"}))));
        assert!(d.policy_ready());
        assert_eq!(d.value["policy"], json!({"redirect_url":""}));
    }
    #[test]
    fn editing_invalidates_review_and_discards_a_late_response() {
        let mut d = Draft {
            value: json!({"name":"Original"}),
            ..Default::default()
        };
        let (old, original) = d.begin_review().unwrap();
        assert_eq!(original["name"], "Original");
        d.edit(|v| v["name"] = json!("Edited"));
        let (current, edited) = d.begin_review().unwrap();
        assert_eq!(edited["name"], "Edited");
        assert!(!d.reviewed(old, Ok(review(json!({"total":500})))));
        assert!(!d.reviewed(old, Err("Stale validation failure".into())));
        assert_eq!(d.review_request, Some(current));
        assert!(d.reviewed(current, Ok(review(json!({"total":2})))));
        d.edit(|v| v["name"] = json!("New"));
        assert!(d.review.is_none());
    }
    #[test]
    fn returning_to_review_revalidates_unchanged_drafts_and_retry_clears_failure() {
        let mut d = Draft {
            value: json!({"name":"Keep this draft"}),
            ..Default::default()
        };
        let (first, body) = d.begin_review().unwrap();
        assert!(d.launch("not-validated".into()).is_none());
        d.clear_review(); // Back while the request is in flight.
        let (second, same_body) = d.begin_review().unwrap();
        assert_eq!(same_body, body);
        assert!(!d.reviewed(first, Ok(review(json!({"total":100})))));
        assert!(!d.reviewed(first, Err("Old request failed".into())));
        assert!(d.reviewed(second, Err("Template was removed".into())));
        assert!(d.review.is_none());
        assert!(d.review_request.is_none());
        assert_eq!(d.review_error.as_deref(), Some("Template was removed"));
        assert!(d.launch("still-invalid".into()).is_none());
        let (retry, _) = d.begin_review().unwrap();
        assert!(d.review_error.is_none());
        assert!(!d.reviewed(second, Ok(review(json!({"total":100})))));
        assert!(d.reviewed(retry, Ok(review(json!({"total":2})))));
        assert_eq!(d.review.as_ref().unwrap().total, 2);
        d.clear_review(); // Returning again must not use the old server result.
        assert!(d.review.is_none());
        let (revisited, _) = d.begin_review().unwrap();
        assert_ne!(revisited, retry);
        assert_eq!(d.value, body);
    }
    #[test]
    fn lost_launch_response_retains_body_key_and_reviewed_resource_versions() {
        let mut d = Draft {
            value: json!({"name":"Reviewed"}),
            ..Default::default()
        };
        let (request, _) = d.begin_review().unwrap();
        d.reviewed(request,Ok(review(json!({"selection":{"template":{"id":7,"revision":"v1"}},"policy":{"capture_passwords":true}}))));
        let first = d.launch("first-key".into());
        assert!(d.begin_review().is_none());
        d.clear_review();
        assert!(d.review.is_some());
        d.edit(|v| v["name"] = json!("Must not replace an uncertain launch"));
        assert_eq!(d.launch("new-key".into()), first);
        assert_eq!(d.value["name"], "Reviewed");
        d.rejected();
        assert!(d.launch("retry".into()).is_none());
    }
}
