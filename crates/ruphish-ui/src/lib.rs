//! Editing state shared by the browser component and native workflow tests.

use base64::{Engine, engine::general_purpose::STANDARD};
use ruphish_contracts::legacy::{self, Attachment, EmailTemplate};

pub mod session;

/// Exported Go template fields offered by the editor.
pub const VARIABLES: &[&str] = &[
    "FirstName",
    "LastName",
    "Email",
    "Position",
    "From",
    "URL",
    "RId",
    "BaseURL",
    "TrackingURL",
    "Tracker",
];

/// A local editing session; the saved value is a cancel checkpoint, not server state.
#[derive(Debug, Clone)]
pub struct Draft {
    checkpoint: EmailTemplate,
    value: EmailTemplate,
}

impl Draft {
    /// Open an existing public template without parsing its HTML.
    pub fn new(value: EmailTemplate) -> Self {
        Self {
            checkpoint: value.clone(),
            value,
        }
    }
    /// Current editable value.
    pub fn value(&self) -> &EmailTemplate {
        &self.value
    }
    /// Edit fields through one authoritative client record.
    pub fn edit(&mut self, edit: impl FnOnce(&mut EmailTemplate)) {
        edit(&mut self.value);
    }
    /// Whether local content differs from the last checkpoint.
    pub fn is_dirty(&self) -> bool {
        self.value != self.checkpoint
    }
    /// Keep a local checkpoint; persistence is supplied by the future resource workflow.
    pub fn checkpoint(&mut self) {
        self.checkpoint = self.value.clone();
    }
    /// Cancel the local edits, including attachment changes.
    pub fn reset(&mut self) {
        self.value = self.checkpoint.clone();
    }
    /// Append a file while retaining its exact bytes and the legacy MIME fallback.
    pub fn attach(&mut self, name: String, mime_type: String, bytes: &[u8]) {
        self.value
            .attachments
            .get_or_insert_with(Vec::new)
            .push(Attachment {
                name,
                content: STANDARD.encode(bytes),
                mime_type: if mime_type.is_empty() {
                    "application/octet-stream".into()
                } else {
                    mime_type
                },
            });
    }
    /// Remove one selected attachment; duplicate filenames are not conflated.
    pub fn remove_attachment(&mut self, index: usize) -> bool {
        if let Some(files) = &mut self.value.attachments
            && index < files.len()
        {
            files.remove(index);
            true
        } else {
            false
        }
    }
    /// Serialize the current template using the preserved public contract.
    pub fn export(&self) -> Result<Vec<u8>, serde_json::Error> {
        legacy::json(&self.value)
    }
    /// Open an exported template, rejecting invalid JSON without changing a live draft.
    pub fn import(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_slice(bytes)?;
        if !value.is_object() {
            return Err(<serde_json::Error as serde::de::Error>::custom(
                "Expected a template object",
            ));
        }
        serde_json::from_value(value).map(Self::new)
    }

    /// Replace the editing session only after the whole imported record is valid.
    pub fn open(&mut self, bytes: &[u8]) -> Result<(), serde_json::Error> {
        let next = Self::import(bytes)?;
        *self = next;
        Ok(())
    }
}

/// Build an isolated preview document without changing the stored source.
///
/// The browser also applies an empty iframe sandbox, inert interaction, and no referrer.
/// Network resources, scripts, frame loads and form submission are blocked in preview.
pub fn preview_document(source: &str) -> String {
    format!(
        "<!doctype html><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src data: blob:; form-action 'none'; base-uri 'none';\">{source}"
    )
}

#[cfg(test)]
#[path = "../../../tests/editor_workflow.rs"]
mod editor_workflow;
