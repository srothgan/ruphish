//! Confirmed resource records, separate from permissive editable inputs.
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

/// Confirmed identity and revision metadata shared by all resource families.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Metadata {
    /// Server-assigned creator and last-editor attribution.
    #[serde(flatten)]
    pub attribution: crate::attribution::Attribution,
    /// Monotonic configuration revision used for conditional writes.
    pub revision: i64,

    /// Persistent resource identity.
    pub id: i64,
    /// Display name.
    pub name: String,
    /// Last modification, or explicitly unknown.
    #[serde(deserialize_with = "crate::nullable")]
    pub modified_date: Option<String>,
}

/// Complete confirmed resource; editable inputs have separate permissive types.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Resource<T = Content> {
    /// Common identity and revision information.
    #[serde(flatten)]
    pub metadata: Metadata,
    /// Family-specific confirmed fields.
    #[serde(flatten)]
    pub content: T,
}

/// Resource content selected by the endpoint, never guessed from overlapping fields.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum Content {
    /// Recipient group.
    Group(Group),
    /// Email template.
    Template(Template),
    /// Landing page.
    Page(Page),
    /// Static learning page.
    LearningPage(LearningPage),
    /// Sending profile.
    Profile(Profile),
}

impl<T> Resource<T> {
    fn into_resource(self, content: impl FnOnce(T) -> Content) -> Resource {
        Resource {
            metadata: self.metadata,
            content: content(self.content),
        }
    }
}

/// Recipient fields in a complete group response.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Recipient {
    /// Email address.
    pub email: String,
    /// Given name.
    pub first_name: String,
    /// Family name.
    pub last_name: String,
    /// Position or role.
    pub position: String,
}
/// Confirmed group audience.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Group {
    /// Ordered recipient records.
    pub targets: Vec<Recipient>,
}
/// Complete attachment content.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Attachment {
    /// Filename.
    pub name: String,
    /// Media type.
    #[serde(rename = "type")]
    pub mime: String,
    /// Base64 content.
    pub content: String,
}
/// Complete email template.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Template {
    /// Subject expression.
    pub subject: String,
    /// Plain-text source.
    pub text: String,
    /// Authored HTML.
    pub html: String,
    /// Visible sender override.
    pub envelope_sender: String,
    /// Ordered attachments.
    pub attachments: Vec<Attachment>,
}
/// Complete landing page and its optional diagnostics.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Page {
    /// Authored HTML.
    pub html: String,
    /// Post-submission destination.
    pub redirect_url: String,
    /// Advisory findings returned by detail reads and saves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<PageWarning>>,
}
/// Complete static learning content.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LearningPage {
    /// Authored HTML.
    pub html: String,
    /// Opaque public identity.
    pub public_id: String,
    /// Prepared preview returned by detail reads, absent from save acknowledgements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_html: Option<String>,
    /// Advisory findings, when supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<PageWarning>>,
}
/// Custom email header.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Header {
    /// Header name.
    pub key: String,
    /// Header value expression.
    pub value: String,
}
/// Complete sending-profile fields returned to an authorized administrator.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Profile {
    /// Sending interface.
    pub interface_type: String,
    /// SMTP endpoint.
    pub host: String,
    /// Sender mailbox.
    pub from_address: String,
    /// Explicit certificate policy.
    pub ignore_cert_errors: bool,
    /// Custom message headers.
    pub headers: Vec<Header>,
    /// Optional configured service login.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Whether a credential is configured. Its value is never returned.
    pub has_password: bool,
}

/// Decode once and retain the confirmed record. Missing required fields are errors.
pub fn decode(kind: Kind, value: Value) -> Result<Resource, serde_json::Error> {
    match kind {
        Kind::Group => serde_json::from_value::<Resource<Group>>(value)
            .map(|v| v.into_resource(Content::Group)),
        Kind::Template => serde_json::from_value::<Resource<Template>>(value)
            .map(|v| v.into_resource(Content::Template)),
        Kind::Page => {
            serde_json::from_value::<Resource<Page>>(value).map(|v| v.into_resource(Content::Page))
        }
        Kind::LearningPage => serde_json::from_value::<Resource<LearningPage>>(value)
            .map(|v| v.into_resource(Content::LearningPage)),
        Kind::Profile => serde_json::from_value::<Resource<Profile>>(value)
            .map(|v| v.into_resource(Content::Profile)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn page_reads_preserve_diagnostics_and_require_complete_defaults() {
        let page = json!({"revision":1,"created_by":null,"edited_by":null,"edited_at":null,"id":7,"name":"Landing","modified_date":null,"html":"<p>Training</p>","redirect_url":"https://training.example.test/","warnings":[{"code":"unsupported-control","control":2}]});
        let resource = decode(Kind::Page, page.clone()).unwrap();
        assert_eq!(resource.metadata.id, 7);
        assert_eq!(serde_json::to_value(resource).unwrap(), page);
        for missing in ["html", "redirect_url", "modified_date"] {
            let mut broken = page.clone();
            broken.as_object_mut().unwrap().remove(missing);
            assert!(decode(Kind::Page, broken).is_err());
        }
        let mut broken = page;
        broken["redirect_url"] = json!(false);
        assert!(decode(Kind::Page, broken).is_err());
    }

    #[test]
    fn learning_reads_keep_prepared_preview_separate_from_authored_content() {
        let page = json!({"revision":1,"created_by":null,"edited_by":null,"edited_at":null,"id":8,"name":"Learning","modified_date":null,"html":"authored","public_id":"public-page","preview_html":"prepared","warnings":[{"code":"script","control":null}]});
        let resource = decode(Kind::LearningPage, page.clone()).unwrap();
        assert_eq!(serde_json::to_value(resource).unwrap(), page);
        let mut acknowledgement = page;
        acknowledgement
            .as_object_mut()
            .unwrap()
            .remove("preview_html");
        assert!(decode(Kind::LearningPage, acknowledgement).is_ok());
    }

    #[test]
    fn attribution_references_are_complete_typed_response_values() {
        let base = json!({"revision":1,"id":7,"name":"Group","modified_date":null,"targets":[],"created_by":{"id":2,"username":"colleague","deleted":true},"edited_by":null,"edited_at":null});
        let resource = decode(Kind::Group, base.clone()).unwrap();
        assert_eq!(serde_json::to_value(resource).unwrap(), base);
        for invalid in [
            json!(2),
            json!({"id":2,"deleted":true}),
            json!({"id":2,"username":"colleague"}),
            json!({"id":2,"username":"colleague","deleted":"true"}),
        ] {
            let mut value = base.clone();
            value["created_by"] = invalid;
            assert!(decode(Kind::Group, value).is_err());
        }
        let mut unknown = base;
        unknown["created_by"] = json!({"id":99999,"username":null,"deleted":false});
        assert!(decode(Kind::Group, unknown).is_ok());
    }

    #[test]
    fn missing_resource_fields_are_not_valid_blank_drafts() {
        let group = json!({"revision":1,"created_by":null,"edited_by":null,"edited_at":null,"id":1,"name":"Audience","modified_date":"2026-10-21T12:00:00Z","targets":[]});
        assert!(decode(Kind::Group, group.clone()).is_ok());
        for field in [
            "id",
            "revision",
            "name",
            "modified_date",
            "targets",
            "created_by",
            "edited_by",
            "edited_at",
        ] {
            let mut broken = group.clone();
            broken.as_object_mut().map(|v| v.remove(field));
            assert!(decode(Kind::Group, broken).is_err());
        }
        for kind in [Kind::Template, Kind::Page, Kind::Profile] {
            assert!(decode(kind, group.clone()).is_err());
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
