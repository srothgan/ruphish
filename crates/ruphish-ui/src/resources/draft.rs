//! Draft transitions tested independently of browser rendering.

#[cfg(test)]
mod tests;

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;

/// Validate exported wire fields before an import can replace any draft content.
pub fn import_resource(bytes: &[u8], family: &str) -> Result<Value, String> {
    use ruphish_contracts::resources::request as input;
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "Invalid resource JSON")?;
    if !value.is_object() {
        return Err("Expected a resource object".into());
    }
    let valid = match family {
        "groups" => {
            value.get("targets").is_some()
                && serde_json::from_value::<input::Group>(value.clone()).is_ok()
        }
        "templates" => {
            (value.get("text").is_some() || value.get("html").is_some())
                && serde_json::from_value::<input::EmailTemplate>(value.clone()).is_ok()
        }
        "learning-pages" => serde_json::from_value::<
            ruphish_contracts::resources::request::LearningPage,
        >(value.clone())
        .is_ok(),
        "pages" => {
            value.get("html").is_some()
                && serde_json::from_value::<input::LandingPage>(value.clone()).is_ok()
        }
        "smtp" => {
            value.get("host").is_some()
                && serde_json::from_value::<input::SendingProfile>(value.clone()).is_ok()
        }
        _ => false,
    };
    if valid {
        Ok(value)
    } else {
        Err("The JSON fields do not match this resource type".into())
    }
}

/// A complete read record plus its derived editable checkpoint.
/// The projection is built once when a response is accepted and replaced atomically on the next save.
#[derive(Clone, Debug)]
pub struct Confirmed {
    /// Typed server response; attribution and other read-only metadata stay here.
    record: ruphish_contracts::resources::details::Resource,
    fields: Value,
    revision: Option<String>,
}
impl Confirmed {
    /// Immutable response corresponding to this checkpoint.
    pub fn record(&self) -> &ruphish_contracts::resources::details::Resource {
        &self.record
    }

    /// Editable projection for an unsaved duplicate.
    pub fn fields_for_edit(&self) -> Value {
        self.fields.clone()
    }

    /// Project only writable fields into the edit checkpoint.
    pub fn new(
        record: ruphish_contracts::resources::details::Resource,
        revision: Option<String>,
    ) -> Result<Self, serde_json::Error> {
        use ruphish_contracts::resources::details::Content;
        use ruphish_contracts::resources::details::Kind;
        let kind = match &record.content {
            Content::Group(_) => Kind::Group,
            Content::Template(_) => Kind::Template,
            Content::Page(_) => Kind::Page,
            Content::LearningPage(_) => Kind::LearningPage,
            Content::Profile(_) => Kind::Profile,
        };
        let input =
            ruphish_contracts::resources::request::decode(kind, serde_json::to_value(&record)?)?;
        let mut fields = serde_json::to_value(input)?;
        if let Content::Profile(profile) = &record.content {
            // A presentation flag used by credential controls; the request projection excludes it.
            fields["has_password"] = Value::Bool(profile.has_password);
        }
        Ok(Self {
            record,
            fields,
            revision,
        })
    }
}
#[derive(Clone)]
enum Checkpoint {
    New(Value),
    Saved(Box<Confirmed>),
}
/// One incomplete edit and its immutable, typed server-confirmed checkpoint.
#[derive(Clone)]
pub struct ResourceDraft {
    value: Value,
    checkpoint: Checkpoint,
}
impl ResourceDraft {
    /// Start an unsaved draft.
    pub fn new(value: Value) -> Self {
        Self {
            checkpoint: Checkpoint::New(value.clone()),
            value,
        }
    }
    /// Open a complete server response for editing.
    pub fn loaded(confirmed: Confirmed) -> Self {
        Self {
            value: confirmed.fields.clone(),
            checkpoint: Checkpoint::Saved(Box::new(confirmed)),
        }
    }
    fn baseline(&self) -> &Value {
        match &self.checkpoint {
            Checkpoint::New(value) => value,
            Checkpoint::Saved(saved) => &saved.fields,
        }
    }
    /// Current authored value, which may still be incomplete.
    pub fn value(&self) -> &Value {
        &self.value
    }
    /// Last confirmed typed response; absent before the first successful save.
    pub fn confirmed(&self) -> Option<&ruphish_contracts::resources::details::Resource> {
        match &self.checkpoint {
            Checkpoint::New(_) => None,
            Checkpoint::Saved(saved) => Some(&saved.record),
        }
    }
    /// Conditional-write revision supplied with the confirmed response.
    pub fn revision(&self) -> Option<&str> {
        match &self.checkpoint {
            Checkpoint::New(_) => None,
            Checkpoint::Saved(saved) => saved.revision.as_deref(),
        }
    }
    /// Apply a local edit without advancing the server checkpoint.
    pub fn edit(&mut self, edit: impl FnOnce(&mut Value)) {
        edit(&mut self.value);
    }
    /// Blank editing means keep; clearing a saved credential is a separate explicit action.
    pub fn set_profile_password(&mut self, password: String) {
        if password.is_empty() {
            self.clear_profile_password(false);
        } else {
            self.value["password"] = Value::String(password);
        }
    }
    /// Select or cancel explicit credential removal.
    pub fn clear_profile_password(&mut self, clear: bool) {
        if clear {
            self.value["password"] = Value::String(String::new());
        } else if let Some(fields) = self.value.as_object_mut() {
            fields.remove("password");
        }
    }
    /// Whether authored input differs from its checkpoint.
    pub fn dirty(&self) -> bool {
        &self.value != self.baseline()
    }
    /// Navigation can finish only after a confirmed save with no newer edits.
    pub fn completed_id(&self) -> Option<i64> {
        self.confirmed()
            .filter(|_| !self.dirty())
            .map(|r| r.metadata.id)
    }
    /// Restore the authoritative editable checkpoint.
    pub fn cancel(&mut self) {
        self.value = self.baseline().clone();
    }
    /// Validate an import before applying writable fields.
    pub fn import(&mut self, bytes: &[u8], family: &str) -> Result<(), String> {
        let value = import_resource(bytes, family)?;
        self.merge_import(&value, family)
    }
    /// Apply only fields represented by the selected request contract, preserving identity.
    pub fn merge_import(&mut self, value: &Value, family: &str) -> Result<(), String> {
        let kind = match family {
            "groups" => ruphish_contracts::resources::details::Kind::Group,
            "templates" => ruphish_contracts::resources::details::Kind::Template,
            "pages" => ruphish_contracts::resources::details::Kind::Page,
            "learning-pages" => ruphish_contracts::resources::details::Kind::LearningPage,
            "smtp" => ruphish_contracts::resources::details::Kind::Profile,
            _ => return Err("Unknown resource family".into()),
        };
        let mut source = value.clone();
        if let Some(fields) = source.as_object_mut() {
            fields.remove("id");
        }
        let input = ruphish_contracts::resources::request::decode(kind, source)
            .map_err(|_| "Invalid resource fields")?;
        let projected = serde_json::to_value(input).map_err(|_| "Invalid resource fields")?;
        if let Some(fields) = projected.as_object() {
            for (key, field) in fields {
                if key != "id" && value.get(key).is_some() {
                    self.value[key] = field.clone();
                }
            }
        }
        Ok(())
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
        ruphish_contracts::common::json(&self.value)
    }
    /// Adopt a complete response while preserving edits made after submission.
    pub fn saved(&mut self, submitted: &Value, confirmed: Confirmed) {
        let returned = &confirmed.fields;
        if &self.value == submitted {
            self.value = returned.clone();
        } else {
            self.value["id"] = Value::from(confirmed.record.metadata.id);
        }
        if let Some(has_password) = returned.get("has_password") {
            self.value["has_password"] = has_password.clone();
            if self.value.get("password") == submitted.get("password")
                && let Some(fields) = self.value.as_object_mut()
            {
                fields.remove("password");
            }
        }
        self.checkpoint = Checkpoint::Saved(Box::new(confirmed));
    }
}
