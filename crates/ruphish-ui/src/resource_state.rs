//! Draft transitions tested independently of browser rendering.
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
    record: ruphish_contracts::resource_details::Resource,
    fields: Value,
    revision: Option<String>,
}
impl Confirmed {
    /// Immutable response corresponding to this checkpoint.
    pub fn record(&self) -> &ruphish_contracts::resource_details::Resource {
        &self.record
    }

    /// Editable projection for an unsaved duplicate.
    pub fn fields_for_edit(&self) -> Value {
        self.fields.clone()
    }

    /// Project only writable fields into the edit checkpoint.
    pub fn new(
        record: ruphish_contracts::resource_details::Resource,
        revision: Option<String>,
    ) -> Result<Self, serde_json::Error> {
        use ruphish_contracts::resource_details::{Content, Kind};
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
    pub fn confirmed(&self) -> Option<&ruphish_contracts::resource_details::Resource> {
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
            "groups" => ruphish_contracts::resource_details::Kind::Group,
            "templates" => ruphish_contracts::resource_details::Kind::Template,
            "pages" => ruphish_contracts::resource_details::Kind::Page,
            "learning-pages" => ruphish_contracts::resource_details::Kind::LearningPage,
            "smtp" => ruphish_contracts::resource_details::Kind::Profile,
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

#[cfg(test)]
mod tests {
    use super::*;
    use ruphish_contracts::{
        attribution::UserReference,
        resource_details::{Content, Group, Metadata, Profile, Resource},
    };
    use serde_json::json;

    fn confirmed(id: i64, name: &str, revision: &str, content: Content) -> Confirmed {
        Confirmed::new(
            Resource {
                metadata: Metadata {
                    id,
                    name: name.into(),
                    revision: 1,
                    modified_date: Some(revision.into()),
                    attribution: Default::default(),
                },
                content,
            },
            Some(revision.into()),
        )
        .unwrap()
    }
    fn profile(has_password: bool) -> Content {
        Content::Profile(Profile {
            interface_type: "SMTP".into(),
            host: "relay.example.test".into(),
            from_address: "sender@example.test".into(),
            username: None,
            has_password,
            ignore_cert_errors: false,
            headers: vec![],
        })
    }
    #[test]
    fn confirmed_password_is_discarded_even_when_another_field_has_newer_edits() {
        let mut draft = ResourceDraft::loaded(confirmed(8, "Relay", "v1", profile(true)));
        draft.set_profile_password("replacement".into());
        let submitted = draft.value().clone();
        draft.edit(|value| value["name"] = json!("Newer name"));
        draft.saved(&submitted, confirmed(8, "Relay", "v2", profile(true)));
        assert!(draft.value().get("password").is_none());
        assert_eq!(draft.value()["name"], "Newer name");
        draft.cancel();
        assert_eq!(draft.value()["name"], "Relay");
        assert!(draft.value().get("password").is_none());
    }
    #[test]
    fn keep_clear_and_a_newer_password_remain_distinct_during_save_and_import() {
        let mut draft = ResourceDraft::loaded(confirmed(8, "Relay", "v1", profile(true)));
        draft.set_profile_password(String::new());
        assert!(draft.value().get("password").is_none());
        draft.clear_profile_password(true);
        assert_eq!(draft.value()["password"], "");
        let submitted = draft.value().clone();
        draft.set_profile_password("newer replacement".into());
        draft.saved(&submitted, confirmed(8, "Relay", "v2", profile(false)));
        assert_eq!(draft.value()["password"], "newer replacement");
        assert_eq!(draft.value()["has_password"], false);
        draft
            .merge_import(
                &json!({"id":99,"has_password":true,"host":"other.example.test"}),
                "smtp",
            )
            .unwrap();
        assert_eq!(draft.value()["id"], 8);
        assert_eq!(draft.value()["host"], "other.example.test");
        assert_eq!(draft.value()["has_password"], false);
        draft.cancel();
        assert!(draft.value().get("password").is_none());
        assert_eq!(draft.value()["has_password"], false);
    }
    #[test]
    fn late_save_preserves_newer_edits_and_cancel_uses_the_server_checkpoint() {
        let mut d = ResourceDraft::loaded(confirmed(
            1,
            "A",
            "old",
            Content::Group(Group { targets: vec![] }),
        ));
        d.edit(|v| v["name"] = json!("Submitted"));
        let submitted = d.value().clone();
        d.edit(|v| v["name"] = json!("Newer edit"));
        let mut saved = confirmed(
            1,
            "Submitted",
            "new",
            Content::Group(Group { targets: vec![] }),
        );
        saved.record.metadata.attribution.edited_by = Some(UserReference {
            id: 2,
            username: Some("bob".into()),
            deleted: false,
        });
        d.saved(&submitted, saved);
        assert_eq!(d.value()["name"], "Newer edit");
        assert!(d.dirty());
        assert_eq!(d.revision(), Some("new"));
        assert_eq!(
            d.confirmed()
                .unwrap()
                .metadata
                .attribution
                .edited_by
                .as_ref()
                .unwrap()
                .username
                .as_deref(),
            Some("bob")
        );
        d.merge_import(
            &json!({"created_at":"forged","created_by":999,"edited_by":999,"edited_at":"forged"}),
            "groups",
        )
        .unwrap();
        for field in [
            "created_at",
            "created_by",
            "edited_by",
            "edited_at",
            "revision",
            "modified_date",
        ] {
            assert!(
                d.value().get(field).is_none(),
                "server metadata cannot enter writable fields"
            );
        }
        assert_eq!(
            d.confirmed()
                .unwrap()
                .metadata
                .attribution
                .edited_by
                .as_ref()
                .unwrap()
                .id,
            2
        );
        d.cancel();
        assert_eq!(d.value()["name"], "Submitted");
        assert!(!d.dirty());
    }
    #[test]
    fn server_normalization_becomes_the_checkpoint_after_success() {
        let value = json!({"id":0,"name":"  Example  ","targets":[]});
        let mut d = ResourceDraft::new(value.clone());
        d.saved(
            &value,
            confirmed(
                2,
                "Example",
                "v1",
                Content::Group(Group { targets: vec![] }),
            ),
        );
        assert_eq!(d.value()["name"], "Example");
        assert_eq!(d.value()["id"], 2);
        assert!(!d.dirty());
    }
}
