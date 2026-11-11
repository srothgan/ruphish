//! Settings edits retain the revision the administrator actually reviewed.
use ruphish_contracts::settings::{Policy, Settings};

/// One editing baseline and its policy/numeric drafts.
#[derive(Clone, Debug)]
pub struct Draft {
    baseline: Settings,
    /// Editable policy switches; numeric values are parsed from the text fields.
    pub policy: Policy,
    /// Preserve invalid or partially entered lifetime text until explicit reload.
    pub validity: String,
    /// Preserve invalid or partially entered interval text until explicit reload.
    pub recheck: String,
}
impl Draft {
    /// Initialize or explicitly replace edits with confirmed server values.
    pub fn from_saved(saved: Settings) -> Self {
        Self {
            validity: saved.policy.verification_validity_seconds.to_string(),
            recheck: saved.policy.verification_recheck_seconds.to_string(),
            policy: saved.policy.clone(),
            baseline: saved,
        }
    }
    /// The revision and parsed values that may be submitted together.
    pub fn edited(&self) -> Option<(i64, Policy)> {
        let mut policy = self.policy.clone();
        policy.verification_validity_seconds = self.validity.parse().ok()?;
        policy.verification_recheck_seconds = self.recheck.parse().ok()?;
        Some((self.baseline.revision, policy))
    }
    /// Dirty state is relative to the reviewed baseline, never a later response.
    pub fn dirty(&self) -> bool {
        self.edited()
            .is_none_or(|(_, policy)| policy != self.baseline.policy)
    }
    /// Newer confirmed values require explicit review before overwriting them.
    pub fn conflicts(&self, saved: &Settings) -> bool {
        saved.revision > self.baseline.revision
    }
    /// A successful refresh updates clean edits atomically; a failed read does nothing.
    pub fn refresh(&mut self, saved: Option<Settings>) {
        if let Some(saved) = saved
            && saved.revision >= self.baseline.revision
            && !self.dirty()
        {
            *self = Self::from_saved(saved);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings(revision: i64) -> Settings {
        Settings {
            revision,
            policy: Policy::default(),
            updated_at: 0,
            updated_by: None,
        }
    }
    #[test]
    fn clean_refresh_tracks_another_administrator_without_becoming_dirty() {
        let mut draft = Draft::from_saved(settings(1));
        let mut remote = settings(2);
        remote.policy.verification_validity_seconds = 172800;
        remote.policy.verification_recheck_seconds = 43200;
        remote.policy.webhooks_enabled = true;
        draft.refresh(Some(remote.clone()));
        assert_eq!(draft.edited(), Some((2, remote.policy)));
        assert_eq!(draft.validity, "172800");
        assert_eq!(draft.recheck, "43200");
        assert!(!draft.dirty());
    }
    #[test]
    fn dirty_and_invalid_input_survives_refresh_until_explicit_reload() {
        let mut draft = Draft::from_saved(settings(1));
        draft.policy.webhooks_enabled = true;
        draft.validity = "unfinished".into();
        let mut remote = settings(2);
        remote.policy.reporting_mailboxes_enabled = true;
        draft.refresh(None);
        draft.refresh(Some(remote.clone()));
        assert!(draft.dirty());
        assert!(draft.conflicts(&remote));
        assert!(draft.policy.webhooks_enabled);
        assert_eq!(draft.validity, "unfinished");
        draft.validity = "86400".into();
        assert_eq!(draft.edited().map(|(revision, _)| revision), Some(1));
        draft = Draft::from_saved(remote.clone());
        assert!(!draft.conflicts(&remote));
        assert!(!draft.dirty());
        draft.policy.webhooks_enabled = true;
        assert_eq!(draft.edited().map(|(revision, _)| revision), Some(2));
    }
    #[test]
    fn confirmed_save_replaces_numeric_text_and_rejects_older_reads() {
        let mut saved = settings(3);
        saved.policy.verification_validity_seconds = 172800;
        let mut draft = Draft::from_saved(saved.clone());
        draft.refresh(Some(settings(2)));
        draft.refresh(None);
        assert_eq!(draft.edited(), Some((3, saved.policy)));
        assert_eq!(draft.validity, "172800");
        assert!(!draft.dirty());
    }
}
