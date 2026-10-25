//! Temporary catalog choices. Preview options are independent of import selection.
use std::collections::{BTreeMap, BTreeSet};

/// Shared logo draft. Replacements keep the last usable image until preparation succeeds.
#[derive(Clone, Default)]
pub struct LogoUpload {
    generation: u64,
    /// Prepared PNG data URL, empty when no image is selected.
    pub data: String,
    /// Original filename for the administrator's reference.
    pub name: String,
    /// Whether the latest chosen image is being prepared.
    pub loading: bool,
    /// Whether preparation of the latest image failed.
    pub failed: bool,
}

impl LogoUpload {
    /// Start a replacement without losing the current image.
    pub fn begin(&mut self) -> u64 {
        self.generation += 1;
        self.loading = true;
        self.failed = false;
        self.generation
    }

    /// Accept only the latest preparation; failures preserve the current image and filename.
    pub fn finish(&mut self, generation: u64, name: String, data: Option<String>) {
        if generation != self.generation {
            return;
        }
        self.loading = false;
        self.failed = data.is_none();
        if let Some(data) = data {
            self.data = data;
            self.name = name;
        }
    }

    /// Remove the image and invalidate any unfinished preparation.
    pub fn clear(&mut self) {
        self.generation += 1;
        self.data.clear();
        self.name.clear();
        self.loading = false;
        self.failed = false;
    }
}

/// Neighbours in the current filtered catalog, independent of import selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewPosition {
    /// One-based position shown in the dialog.
    pub number: usize,
    /// Number of entries in this filtered catalog.
    pub total: usize,
    /// Previous entry, absent at the beginning.
    pub previous: Option<String>,
    /// Next entry, absent at the end.
    pub next: Option<String>,
}

impl ReviewPosition {
    /// Resolve current neighbours without wrapping or navigating outside the filter.
    pub fn in_list(ids: &[String], current: &str) -> Option<Self> {
        let index = ids.iter().position(|id| id == current)?;
        Some(Self {
            number: index + 1,
            total: ids.len(),
            previous: index
                .checked_sub(1)
                .and_then(|index| ids.get(index))
                .cloned(),
            next: ids.get(index + 1).cloned(),
        })
    }
}

/// Per-template choices, initially disabled and retained during this page visit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Prefill the campaign recipient's email address.
    pub prefill: bool,
    /// Embed the page's shared uploaded logo when creating a copy.
    pub logo: bool,
}

/// Selection owns membership only; deselection must not discard configured options.
#[derive(Clone, Default)]
pub struct StarterSelection {
    selected: BTreeSet<String>,
    options: BTreeMap<String, Options>,
}

impl StarterSelection {
    /// Whether this entry will be included in the next addition.
    pub fn contains(&self, id: &str) -> bool {
        self.selected.contains(id)
    }

    /// Change membership without changing preview settings.
    pub fn select(&mut self, id: &str, selected: bool) {
        if selected {
            self.selected.insert(id.to_owned());
        } else {
            self.selected.remove(id);
        }
    }

    /// Read configured options, including for an unselected template.
    pub fn options(&self, id: &str) -> Options {
        self.options.get(id).copied().unwrap_or_default()
    }

    /// Configure a template without selecting it for creation.
    pub fn configure(&mut self, id: &str, edit: impl FnOnce(&mut Options)) {
        edit(self.options.entry(id.to_owned()).or_default());
    }

    /// Number selected across all catalog tabs.
    pub fn count(&self) -> usize {
        self.selected.len()
    }

    /// Only selected entries can require a logo before adding copies.
    pub fn needs_logo(&self) -> bool {
        self.selected.iter().any(|id| self.options(id).logo)
    }

    /// Freeze selection and options for a batch of independent creates.
    pub fn snapshot(&self) -> Vec<(String, Options)> {
        self.selected
            .iter()
            .map(|id| (id.clone(), self.options(id)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_logo_replacement_preserves_the_usable_image() {
        let mut upload = LogoUpload::default();
        let first = upload.begin();
        upload.finish(first, "company.png".into(), Some("prepared image".into()));
        let replacement = upload.begin();
        assert_eq!(upload.data, "prepared image");
        upload.finish(replacement, "broken.png".into(), None);
        assert!(upload.failed);
        assert!(!upload.loading);
        assert_eq!(upload.name, "company.png");
        assert_eq!(upload.data, "prepared image");
    }

    #[test]
    fn logo_replacement_and_removal_reject_late_completions() {
        let mut upload = LogoUpload::default();
        let first = upload.begin();
        let second = upload.begin();
        upload.finish(first, "old.png".into(), Some("old".into()));
        assert!(upload.loading);
        assert!(upload.data.is_empty());
        upload.finish(second, "new.png".into(), Some("new".into()));
        upload.finish(first, "old.png".into(), None);
        assert!(!upload.failed);
        assert_eq!(upload.data, "new");
        let third = upload.begin();
        upload.clear();
        upload.finish(third, "removed.png".into(), Some("removed".into()));
        assert!(upload.data.is_empty());
        assert!(upload.name.is_empty());
        assert!(!upload.loading);
        assert!(!upload.failed);
        let fourth = upload.begin();
        upload.finish(fourth, "chosen-again.png".into(), Some("restored".into()));
        assert_eq!(upload.data, "restored");
    }

    #[test]
    fn review_follows_filtered_order_and_stops_at_boundaries() {
        let ids = vec!["zoom".into(), "microsoft".into(), "google".into()];
        assert_eq!(
            ReviewPosition::in_list(&ids, "microsoft"),
            Some(ReviewPosition {
                number: 2,
                total: 3,
                previous: Some("zoom".into()),
                next: Some("google".into()),
            })
        );
        let filtered = vec!["google".into(), "microsoft".into()];
        assert_eq!(
            ReviewPosition::in_list(&filtered, "google"),
            Some(ReviewPosition {
                number: 1,
                total: 2,
                previous: None,
                next: Some("microsoft".into()),
            })
        );
        assert_eq!(
            ReviewPosition::in_list(&filtered, "microsoft"),
            Some(ReviewPosition {
                number: 2,
                total: 2,
                previous: Some("google".into()),
                next: None,
            })
        );
        assert_eq!(
            ReviewPosition::in_list(&["zoom".into()], "zoom"),
            Some(ReviewPosition {
                number: 1,
                total: 1,
                previous: None,
                next: None,
            })
        );
        assert!(ReviewPosition::in_list(&filtered, "zoom").is_none());
        assert!(ReviewPosition::in_list(&[], "zoom").is_none());
    }

    #[test]
    fn configure_before_selecting_and_deselect_without_losing_preview_choices() {
        let mut draft = StarterSelection::default();
        assert_eq!(draft.options("microsoft"), Options::default());
        draft.configure("microsoft", |options| {
            options.prefill = true;
            options.logo = true;
        });
        assert!(draft.options("microsoft").prefill);
        assert!(!draft.contains("microsoft"));
        assert!(
            !draft.needs_logo(),
            "unselected previews cannot block other imports"
        );
        assert!(draft.snapshot().is_empty());
        draft.select("microsoft", true);
        assert!(draft.needs_logo());
        assert_eq!(
            draft.snapshot(),
            vec![(
                "microsoft".into(),
                Options {
                    prefill: true,
                    logo: true
                }
            )]
        );
        draft.select("microsoft", false);
        assert!(!draft.needs_logo());
        assert!(draft.options("microsoft").logo);
        draft.select("microsoft", true);
        assert!(draft.snapshot()[0].1.prefill);
    }

    #[test]
    fn imports_snapshot_only_selected_options_and_do_not_reset_other_previews() {
        let mut draft = StarterSelection::default();
        draft.configure("zoom", |options| options.logo = true);
        draft.configure("google", |options| options.prefill = true);
        draft.select("google", true);
        draft.select("email", true);
        draft.select("learning", true);
        assert_eq!(draft.count(), 3);
        assert!(!draft.needs_logo());
        let batch = draft.snapshot();
        draft.configure("google", |options| options.prefill = false);
        draft.configure("learning", |options| options.logo = true);
        assert!(draft.needs_logo());
        assert_eq!(
            batch,
            vec![
                ("email".into(), Options::default()),
                (
                    "google".into(),
                    Options {
                        prefill: true,
                        logo: false
                    }
                ),
                ("learning".into(), Options::default()),
            ]
        );
        for (id, _) in batch {
            draft.select(&id, false);
        }
        assert_eq!(draft.count(), 0);
        assert!(!draft.needs_logo());
        assert!(draft.options("learning").logo);
        assert!(draft.options("zoom").logo);
    }
}
