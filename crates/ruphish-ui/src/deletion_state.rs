//! Confirmation and single-flight ownership for resource deletion.

/// Snapshot of the resource the operator chose to delete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteTarget {
    /// Stable identity returned to the list after successful deletion.
    pub id: i64,
    /// Name shown in the confirmation.
    pub name: String,
    /// Resource API endpoint.
    pub path: String,
    /// Optional revision sent as an If-Match precondition.
    pub revision: Option<String>,
    /// Feature-specific consequences and restrictions.
    pub description: &'static str,
}

/// Page-owned selection and in-flight deletion.
#[derive(Clone, Default)]
pub struct Deletion {
    target: Option<DeleteTarget>,
    busy: bool,
}
impl Deletion {
    /// Resource awaiting confirmation or completion.
    pub fn target(&self) -> Option<&DeleteTarget> {
        self.target.as_ref()
    }
    /// Whether a request currently owns the selection.
    pub fn busy(&self) -> bool {
        self.busy
    }
    /// Choose a resource while no deletion is in flight.
    pub fn select(&mut self, target: DeleteTarget) {
        if !self.busy {
            self.target = Some(target);
        }
    }
    /// Dismiss an idle confirmation without changing persisted data.
    pub fn cancel(&mut self) {
        if !self.busy {
            self.target = None;
        }
    }
    /// Admit one request and freeze its selected resource and revision.
    pub fn begin(&mut self) -> Option<DeleteTarget> {
        if self.busy {
            return None;
        }
        let target = self.target.clone()?;
        self.busy = true;
        Some(target)
    }
    /// Failed requests retain the original selection for retry or cancellation.
    pub fn finish(&mut self, succeeded: bool) -> Option<i64> {
        if !self.busy {
            return None;
        }
        self.busy = false;
        if succeeded {
            self.target.take().map(|target| target.id)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn target(id: i64) -> DeleteTarget {
        DeleteTarget {
            id,
            name: format!("Resource {id}"),
            path: format!("/api/domains/{id}"),
            revision: Some("\"7\"".into()),
            description: "Delete this resource.",
        }
    }
    #[test]
    fn confirmation_is_single_flight_and_cannot_change_its_target_mid_request() {
        let mut deletion = Deletion::default();
        assert_eq!(deletion.begin(), None);
        deletion.select(target(1));
        assert_eq!(deletion.begin(), Some(target(1)));
        assert!(deletion.busy());
        assert_eq!(deletion.begin(), None);
        deletion.select(target(2));
        deletion.cancel();
        assert_eq!(deletion.target(), Some(&target(1)));
        assert_eq!(deletion.finish(true), Some(1));
        assert!(!deletion.busy());
        assert_eq!(deletion.target(), None);
        assert_eq!(deletion.finish(true), None);
        assert_eq!(deletion.begin(), None);
    }
    #[test]
    fn failed_request_preserves_revision_for_retry_and_allows_cancel_or_new_selection() {
        let mut deletion = Deletion::default();
        deletion.select(target(1));
        assert_eq!(deletion.begin(), Some(target(1)));
        assert_eq!(deletion.finish(false), None);
        assert!(!deletion.busy());
        assert_eq!(deletion.target(), Some(&target(1)));
        assert_eq!(deletion.begin(), Some(target(1)));
        deletion.finish(false);
        deletion.cancel();
        assert_eq!(deletion.target(), None);
        deletion.select(target(2));
        assert_eq!(deletion.begin(), Some(target(2)));
        assert_eq!(deletion.finish(true), Some(2));
    }
}
