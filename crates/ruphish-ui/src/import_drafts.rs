//! Tab-local imported drafts owned by the authenticated shell, never browser storage.
use crate::{
    import_state::{ImportError, ImportReview},
    resource_state::ResourceDraft,
};
use leptos::prelude::*;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = "export function importDraftId() { return crypto.randomUUID(); }")]
extern "C" {
    #[wasm_bindgen(catch, js_name = importDraftId)]
    fn random_id() -> Result<String, JsValue>;
}

/// References share one editable value between the store and the mounted form.
#[derive(Clone)]
pub struct ImportedDraft {
    family: String,
    /// The sole editable draft and server checkpoint.
    pub draft: ArcRwSignal<ResourceDraft>,
    /// Review notes and explicit acknowledgement of CSV omissions.
    pub review: ArcRwSignal<ImportReview>,
}

/// The shell's lifetime scopes access and clears all draft references on identity changes.
#[derive(Clone, Copy)]
pub struct ImportDrafts(StoredValue<HashMap<String, ImportedDraft>>);

impl Default for ImportDrafts {
    fn default() -> Self {
        Self(StoredValue::new(HashMap::new()))
    }
}

impl ImportDrafts {
    /// Retain a draft under a cryptographically random, content-free URL identifier.
    pub fn insert(
        self,
        family: &str,
        draft: ResourceDraft,
        review: ImportReview,
    ) -> Result<String, ImportError> {
        let id = random_id().map_err(|_| ImportError::IdUnavailable)?;
        self.0
            .try_update_value(|drafts| {
                if drafts.len() >= 8 || drafts.contains_key(&id) {
                    return Err(ImportError::Capacity);
                }
                drafts.insert(
                    id.clone(),
                    ImportedDraft {
                        family: family.into(),
                        draft: ArcRwSignal::new(draft),
                        review: ArcRwSignal::new(review),
                    },
                );
                Ok(id)
            })
            .unwrap_or(Err(ImportError::DraftUnavailable))
    }

    /// Resolve only a matching resource family in this authenticated shell.
    pub fn get(self, family: &str, id: &str) -> Option<ImportedDraft> {
        self.0
            .try_with_value(|drafts| {
                drafts
                    .get(id)
                    .filter(|draft| draft.family == family)
                    .cloned()
            })
            .flatten()
    }

    /// Remove a completed or explicitly discarded draft.
    pub fn remove(self, id: &str) {
        self.0.try_update_value(|drafts| {
            drafts.remove(id);
        });
    }
}
