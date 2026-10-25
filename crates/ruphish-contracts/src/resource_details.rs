//! Strict read validation, separate from permissive legacy write decoders.
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Preserved resource endpoint family.
#[derive(Clone, Copy)]
pub enum Kind {
    /// Recipient groups.
    Group,
    /// Email templates.
    Template,
    /// Landing pages.
    Page,
    /// Static learning pages.
    LearningPage,
    /// Sending profiles.
    Profile,
}

#[derive(Deserialize, Serialize)]
struct Detail<T> {
    id: i64,
    name: String,
    modified_date: String,
    #[serde(flatten)]
    content: T,
}
#[derive(Deserialize, Serialize)]
struct Recipient {
    email: String,
    first_name: String,
    last_name: String,
    position: String,
}
#[derive(Deserialize, Serialize)]
struct Group {
    targets: Vec<Recipient>,
}
#[derive(Deserialize, Serialize)]
struct Attachment {
    name: String,
    #[serde(rename = "type")]
    mime: String,
    content: String,
}
#[derive(Deserialize, Serialize)]
struct Template {
    subject: String,
    text: String,
    html: String,
    envelope_sender: String,
    attachments: Vec<Attachment>,
}
#[derive(Deserialize, Serialize)]
struct Page {
    html: String,
    capture_credentials: bool,
    capture_passwords: bool,
    redirect_url: String,
}
#[derive(Deserialize, Serialize)]
struct LearningPage {
    html: String,
    public_id: String,
}
#[derive(Deserialize, Serialize)]
struct Header {
    key: String,
    value: String,
}
#[derive(Deserialize, Serialize)]
struct Profile {
    interface_type: String,
    host: String,
    from_address: String,
    ignore_cert_errors: bool,
    headers: Vec<Header>,
    username: Option<String>,
    password: Option<String>,
}

/// Reject missing required data before it enters a read cache or editable draft.
/// Optional SMTP credentials may be omitted by the preserved API.
pub fn validate(kind: Kind, value: &Value) -> Result<(), serde_json::Error> {
    match kind {
        Kind::Group => serde_json::from_value::<Detail<Group>>(value.clone()).map(|_| ()),
        Kind::Template => serde_json::from_value::<Detail<Template>>(value.clone()).map(|_| ()),
        Kind::LearningPage => {
            serde_json::from_value::<Detail<LearningPage>>(value.clone()).map(|_| ())
        }
        Kind::Page => serde_json::from_value::<Detail<Page>>(value.clone()).map(|_| ()),
        Kind::Profile => serde_json::from_value::<Detail<Profile>>(value.clone()).map(|_| ()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn missing_resource_fields_are_not_valid_blank_drafts() {
        let group =
            json!({"id":1,"name":"Audience","modified_date":"2026-10-21T12:00:00Z","targets":[]});
        assert!(validate(Kind::Group, &group).is_ok());
        for field in ["id", "name", "modified_date", "targets"] {
            let mut broken = group.clone();
            broken.as_object_mut().map(|v| v.remove(field));
            assert!(validate(Kind::Group, &broken).is_err());
        }
        for kind in [Kind::Template, Kind::Page, Kind::Profile] {
            assert!(validate(kind, &group).is_err());
        }
    }
}

/// Advisory landing-page finding; no participant values or authored markup.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageWarning {
    /// Stable diagnostic code translated by the UI.
    pub code: String,
    /// One-based input position in the processed document.
    pub control: Option<usize>,
}
