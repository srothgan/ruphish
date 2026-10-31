//! A pending request is immutable until its outcome is known.
use ruphish_domain::event_edit::{Action, Change, Outcome};

#[derive(Clone, Default)]
/// Draft state survives refresh, validation failures and concurrent-edit rejection.
pub struct Editor {
    /// Command currently being reviewed.
    pub draft: Option<Change>,
    /// Frozen command used for uncertain-response retries.
    pub pending: Option<Change>,
    /// A transport request is active.
    pub busy: bool,
    /// Failure explanation.
    pub error: String,
}
impl Editor {
    /// Snapshot a row only on explicit user action, never on a background refresh.
    pub fn open(&mut self, change: Change) {
        if self.busy || self.pending.is_some() {
            return;
        }
        self.draft = Some(change);
        self.error.clear();
    }
    /// Start/retry without generating another identity or replacing reviewed values.
    pub fn begin(&mut self) -> Option<Change> {
        if self.busy {
            return None;
        }
        let request = self.pending.clone().or_else(|| self.draft.clone())?;
        self.pending = Some(request.clone());
        self.busy = true;
        self.error.clear();
        Some(request)
    }
    /// Confirmed rejection unlocks the draft; uncertain errors retain the frozen request.
    pub fn failed(&mut self, message: String, definite: bool) {
        self.busy = false;
        self.error = message;
        if definite {
            self.pending = None;
        }
    }
    /// An acknowledged mutation clears the draft, independently of read refresh success.
    pub fn saved(&mut self, outcome: &Outcome) -> bool {
        let Some(request) = &self.pending else {
            return false;
        };
        if outcome.id <= 0 || request.event_id.is_some_and(|id| id != outcome.id) {
            return false;
        }
        let valid = match (&outcome.event, request.action) {
            (None, Action::Delete) => true,
            (Some(record), Action::Add) => {
                record.id == outcome.id
                    && record.source == "operator"
                    && record.revision == 0
                    && !record.edited
            }
            (Some(record), Action::Edit) => {
                record.id == outcome.id
                    && (record.revision == request.revision
                        || Some(record.revision) == request.revision.checked_add(1))
            }
            _ => false,
        };
        if !valid {
            return false;
        }
        *self = Self::default();
        true
    }
    /// Cancelling an uncertain addition could create duplicates on a later attempt.
    pub fn cancel(&mut self) {
        if !self.busy && self.pending.is_none() {
            *self = Self::default();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ruphish_domain::event_edit::{Action, Presence, Record, Value};
    use ruphish_domain::events::Kind;
    #[test]
    fn unknown_outcomes_retry_identical_commands_and_conflicts_keep_input() {
        let change = Change {
            request_id: "independent-retry-key".into(),
            event_id: Some(5),
            revision: 2,
            action: Action::Edit,
            value: Some(Value {
                kind: Kind::LinkClicked,
                time: "2026-01-01T12:00:00Z".into(),
                presence: Presence::default(),
            }),
        };
        let mut editor = Editor::default();
        editor.open(change.clone());
        assert_eq!(editor.begin(), Some(change.clone()));
        editor.failed("lost acknowledgement".into(), false);
        editor.cancel();
        assert!(editor.draft.is_some());
        assert_eq!(editor.begin(), Some(change.clone()));
        editor.failed("revision conflict".into(), true);
        assert_eq!(editor.draft, Some(change.clone()));
        assert!(editor.pending.is_none());
        editor.begin();
        let fact = ruphish_domain::event_edit::Fact {
            time: "2026-01-01T12:00:00Z".into(),
            kind: ruphish_domain::events::RecordedKind::Known(Kind::LinkClicked),
            presence: Presence::default(),
        };
        let mut acknowledgement = Record {
            client: None,
            id: 999,
            source: "public".into(),
            revision: 3,
            current: fact,
            edited: true,
        };
        assert!(!editor.saved(&Outcome {
            id: acknowledgement.id,
            event: Some(acknowledgement.clone())
        }));
        assert_eq!(editor.draft, Some(change));
        acknowledgement.id = 5;
        assert!(editor.saved(&Outcome {
            id: acknowledgement.id,
            event: Some(acknowledgement.clone())
        }));
        editor.cancel();
        assert!(editor.draft.is_none());
    }
}
