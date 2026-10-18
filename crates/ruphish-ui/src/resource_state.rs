//! Draft transitions tested independently of browser rendering.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;

/// Validate exported wire fields before an import can replace any draft content.
pub fn import_resource(bytes: &[u8], family: &str) -> Result<Value, String> {
    use ruphish_contracts::legacy;
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "Invalid resource JSON")?;
    if !value.is_object() {
        return Err("Expected a resource object".into());
    }
    let valid = match family {
        "groups" => {
            value.get("targets").is_some()
                && serde_json::from_value::<legacy::Group>(value.clone()).is_ok()
        }
        "templates" => {
            (value.get("text").is_some() || value.get("html").is_some())
                && serde_json::from_value::<legacy::EmailTemplate>(value.clone()).is_ok()
        }
        "pages" => {
            value.get("html").is_some()
                && serde_json::from_value::<legacy::LandingPage>(value.clone()).is_ok()
        }
        "smtp" => {
            value.get("host").is_some()
                && serde_json::from_value::<legacy::SendingProfile>(value.clone()).is_ok()
        }
        _ => false,
    };
    if valid {
        Ok(value)
    } else {
        Err("The JSON fields do not match this resource type".into())
    }
}

/// One draft plus its last server-confirmed checkpoint and revision.
#[derive(Clone, Debug)]
pub struct ResourceDraft {
    value: Value,
    saved: Value,
    revision: Option<String>,
}
impl ResourceDraft {
    /// Open a new or persisted resource without a second editable copy.
    pub fn new(value: Value, revision: Option<String>) -> Self {
        Self {
            saved: value.clone(),
            value,
            revision,
        }
    }
    /// Current authored value.
    pub fn value(&self) -> &Value {
        &self.value
    }
    /// Server revision used for conditional writes.
    pub fn revision(&self) -> Option<&str> {
        self.revision.as_deref()
    }
    /// Apply a local edit without advancing the server checkpoint.
    pub fn edit(&mut self, edit: impl FnOnce(&mut Value)) {
        edit(&mut self.value);
    }
    /// Whether the draft differs from the server checkpoint.
    pub fn dirty(&self) -> bool {
        self.value != self.saved
    }
    /// Navigation may finish only after a persisted save with no newer draft edits.
    pub fn completed_id(&self) -> Option<i64> {
        (!self.dirty())
            .then(|| self.saved["id"].as_i64())
            .flatten()
            .filter(|id| *id > 0)
    }
    /// Restore only the last confirmed value.
    pub fn cancel(&mut self) {
        self.value = self.saved.clone();
    }
    /// Validate all supplied fields before replacing any authored content, retaining identity.
    pub fn import(&mut self, bytes: &[u8], family: &str) -> Result<(), String> {
        let value = import_resource(bytes, family)?;
        self.merge_import(&value);
        Ok(())
    }
    /// Apply reviewed fields while retaining this resource's identity and server revision.
    pub fn merge_import(&mut self, value: &Value) {
        if let Some(fields) = value.as_object() {
            for (key, value) in fields {
                if !matches!(key.as_str(), "id" | "modified_date" | "user_id") {
                    self.value[key] = value.clone();
                }
            }
        }
    }
    /// Add a binary file without conflating duplicate names or changing the checkpoint.
    pub fn attach(&mut self, name: String, mime_type: String, bytes: &[u8]) {
        if !self.value["attachments"].is_array() {
            self.value["attachments"] = serde_json::json!([]);
        }
        if let Some(files) = self.value["attachments"].as_array_mut() {
            files.push(serde_json::json!({"name":name,"type":if mime_type.is_empty(){"application/octet-stream"}else{&mime_type},"content":STANDARD.encode(bytes)}));
        }
    }
    /// Remove exactly one selected attachment; an out-of-range selection changes nothing.
    pub fn remove_attachment(&mut self, index: usize) -> bool {
        if let Some(files) = self.value["attachments"].as_array_mut()
            && index < files.len()
        {
            files.remove(index);
            true
        } else {
            false
        }
    }
    /// Export the authored wire representation without losing attachment bytes.
    pub fn export(&self) -> Result<Vec<u8>, serde_json::Error> {
        ruphish_contracts::legacy::json(&self.value)
    }
    /// Adopt normalized saved content unless the user has made newer edits.
    pub fn saved(&mut self, submitted: &Value, returned: Value, revision: Option<String>) {
        if &self.value == submitted {
            self.value = returned.clone();
        } else {
            for key in ["id", "modified_date"] {
                self.value[key] = returned[key].clone();
            }
        }
        self.saved = returned;
        self.revision = revision;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn late_save_preserves_newer_edits_and_cancel_uses_the_server_checkpoint() {
        let initial = json!({"id":1,"name":"A","targets":[]});
        let mut d = ResourceDraft::new(initial, Some("old".into()));
        d.edit(|v| v["name"] = json!("Submitted"));
        let submitted = d.value().clone();
        d.edit(|v| v["name"] = json!("Newer edit"));
        d.saved(
            &submitted,
            json!({"id":1,"name":"Submitted","targets":[],"modified_date":"new"}),
            Some("new".into()),
        );
        assert_eq!(d.value()["name"], "Newer edit");
        assert!(d.dirty());
        assert_eq!(d.revision(), Some("new"));
        d.cancel();
        assert_eq!(d.value()["name"], "Submitted");
        assert!(!d.dirty());
    }
    #[test]
    fn server_normalization_becomes_the_checkpoint_after_success() {
        let value = json!({"id":0,"capture_credentials":false,"capture_passwords":true});
        let mut d = ResourceDraft::new(value.clone(), None);
        d.saved(
            &value,
            json!({"id":2,"capture_credentials":true,"capture_passwords":true}),
            Some("v1".into()),
        );
        assert_eq!(d.value()["capture_credentials"], true);
        assert_eq!(d.value()["id"], 2);
        assert!(!d.dirty());
    }
}
