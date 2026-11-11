//! Creation identity and confirmed checkpoints advance through one owner.
use ruphish_contracts::campaign::reads::Draft;
use serde_json::Value;

#[derive(Clone, Default)]
enum Identity {
    #[default]
    New,
    Creating {
        key: String,
        body: Value,
    },
    // Creation is known, but a differing current record has not been adopted.
    Created(i64),
    Saved {
        record: Box<Draft>,
        generation: u64,
    },
}

/// An uncertain creation keeps its identity even when no local fields were edited.
#[derive(Clone, Default)]
pub struct PersistenceState {
    identity: Identity,
}

impl PersistenceState {
    /// Initialize only from a validated server identity and revision.
    pub fn loaded(record: Draft) -> Option<Self> {
        (record.id > 0 && record.revision > 0).then(|| Self {
            identity: Identity::Saved {
                record: Box::new(record),
                generation: 0,
            },
        })
    }
    /// Known identity, including a creation awaiting explicit reconciliation.
    pub fn id(&self) -> Option<i64> {
        match &self.identity {
            Identity::New | Identity::Creating { .. } => None,
            Identity::Created(id) => Some(*id),
            Identity::Saved { record, .. } => Some(record.id),
        }
    }
    /// Never adopt a colleague's revision from a replayed creation response.
    pub fn revision(&self) -> Option<i64> {
        match &self.identity {
            Identity::Created(_) => Some(1),
            Identity::Saved { record, .. } => Some(record.revision),
            _ => None,
        }
    }
    /// Latest confirmed record, atomically paired with its saved edit generation.
    pub fn confirmed(&self) -> Option<&Draft> {
        match &self.identity {
            Identity::Saved { record, .. } => Some(record),
            _ => None,
        }
    }
    /// Generation certified by the last accepted save or explicit reload.
    pub fn saved_generation(&self) -> u64 {
        match self.identity {
            Identity::Saved { generation, .. } => generation,
            _ => 0,
        }
    }
    /// Navigation must protect a creation whose result is not yet known.
    pub fn creating(&self) -> bool {
        matches!(self.identity, Identity::Creating { .. })
    }
    /// An unresolved creation or an unreviewed returned identity must not be discarded silently.
    pub fn unconfirmed(&self) -> bool {
        matches!(
            self.identity,
            Identity::Creating { .. } | Identity::Created(_)
        )
    }
    /// Allocate a key only for a genuinely new creation.
    pub fn is_new(&self) -> bool {
        matches!(self.identity, Identity::New)
    }
    /// Freeze the first request; retries cannot replace its key or content.
    pub fn begin_creation(&mut self, key: String, body: Value) {
        if self.is_new() {
            self.identity = Identity::Creating { key, body };
        }
    }
    /// Exact request to retry while creation is unresolved.
    pub fn creation(&self) -> Option<(String, Value)> {
        match &self.identity {
            Identity::Creating { key, body } => Some((key.clone(), body.clone())),
            _ => None,
        }
    }
    /// Remember a returned identity without certifying intervening server edits.
    pub fn created(&mut self, id: i64) -> bool {
        if !self.creating() || id <= 0 {
            return false;
        }
        self.identity = Identity::Created(id);
        true
    }
    /// A definite creation rejection releases only the uncommitted identity.
    pub fn rejected(&mut self) {
        if self.creating() {
            self.identity = Identity::New;
        }
    }
    /// Accept a save or explicit reload for this identity as one checkpoint.
    pub fn accept(&mut self, record: Draft, generation: u64) -> bool {
        if self.id() != Some(record.id) || record.revision <= 0 {
            return false;
        }
        self.identity = Identity::Saved {
            record: Box::new(record),
            generation,
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn uncertain_creation_keeps_identity_and_requires_review_before_advancing_revision() {
        let mut state = PersistenceState::default();
        let body = json!({"configuration":{"name":"original"},"step":0});
        state.begin_creation("first-key".into(), body.clone());
        state.begin_creation("different-key".into(), json!({"name":"newer edit"}));
        assert_eq!(state.creation(), Some(("first-key".into(), body)));
        assert!(
            state.creating(),
            "an unedited draft still has an unresolved operation"
        );
        assert_eq!(state.revision(), None);
        assert!(state.unconfirmed());
        assert!(state.created(41));
        assert!(state.unconfirmed());
        assert_eq!(state.revision(), Some(1));
        assert!(state.confirmed().is_none());
        let record: Draft = serde_json::from_value(json!({
            "id":41,"revision":3,"configuration":{
                "name":"colleague","url":"","selection":{
                    "groups":[],"template":{"id":0},"page":{"id":0},"smtp":{"id":0}
                },"policy":null,"launch_local":null,"send_by_local":null
            },"step":1,
            "created":null,"modified":null,"campaign_id":null,"status":"draft",
            "created_at":null,"created_by":null,"edited_at":null,"edited_by":null
        }))
        .unwrap();
        let mut wrong = record.clone();
        wrong.id = 42;
        assert!(!state.accept(wrong, 8));
        assert_eq!(state.revision(), Some(1));
        assert!(state.accept(record, 8));
        assert_eq!(state.revision(), Some(3));
        assert_eq!(state.saved_generation(), 8);
        assert_eq!(state.confirmed().unwrap().configuration.name, "colleague");
        assert!(!state.creating());
        assert!(!state.unconfirmed());
    }
}
