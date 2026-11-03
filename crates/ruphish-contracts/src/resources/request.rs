//! Writable resource fields shared by the HTTP boundary and browser.
use serde::{Deserialize, Serialize};

/// A recipient's public fields, without persistence identifiers or ownership.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Recipient {
    /// Email address.
    pub email: String,
    /// First name.
    pub first_name: String,
    /// Last name.
    pub last_name: String,
    /// Position.
    pub position: String,
}

/// Group fields used by resource writes and imports.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Group {
    /// Public group ID.
    pub id: i64,
    /// Group name.
    pub name: String,
    /// Recipient records; an empty collection is an empty array.
    pub targets: Vec<Recipient>,
}

pub use super::fields::{Attachment, Header};

/// Sending-profile write fields; omission keeps the password and an empty password clears it.
#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SendingProfile {
    /// Profile ID.
    pub id: i64,
    /// Sending interface name.
    pub interface_type: String,
    /// Profile name.
    pub name: String,
    /// SMTP host.
    pub host: String,
    /// SMTP login name.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub username: String,
    /// New SMTP credential; omitted or null retains the saved value. Empty explicitly clears it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Sender mailbox.
    pub from_address: String,
    /// Existing certificate-verification setting.
    pub ignore_cert_errors: bool,
    /// Custom headers; empty when none are configured.
    pub headers: Vec<Header>,
}

/// Email-template wire representation; validation belongs to the application.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EmailTemplate {
    /// Public identifier.
    pub id: i64,
    /// Template name.
    pub name: String,
    /// Envelope sender override.
    pub envelope_sender: String,
    /// Subject template.
    pub subject: String,
    /// Plain-text body.
    pub text: String,
    /// Authoritative HTML source.
    pub html: String,
    /// Attachments; empty when none are configured.
    pub attachments: Vec<Attachment>,
}

/// Landing-page wire contract.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LandingPage {
    /// Public identifier.
    pub id: i64,
    /// Shared display name.
    pub name: String,
    /// Authored document source.
    pub html: String,
    /// Templated destination after submission.
    pub redirect_url: String,
}

/// CSV import review input.
#[derive(Serialize, Deserialize)]
pub struct CsvRequest {
    /// Source content supplied for processing.
    pub content: String,
    /// Explicit CSV source-column mapping, when supplied.
    pub columns: Option<[String; 4]>,
}

/// Email import input.
#[derive(Serialize, Deserialize)]
pub struct EmailRequest {
    /// Source content supplied for processing.
    pub content: String,
    #[serde(default)]
    /// Convert imported links to campaign links.
    pub convert_links: bool,
}

/// Site import input.
#[derive(Serialize, Deserialize)]
pub struct SiteRequest {
    /// Configured destination URL.
    pub url: String,
}

/// ZIP page-bundle import input.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageBundleRequest {
    /// Source content supplied for processing.
    pub content: String,
}

/// Page preview input.
#[derive(Serialize, Deserialize)]
pub struct PreviewRequest {
    /// Authored HTML source.
    pub html: String,
    #[serde(default)]
    /// Recipient values used for the preview.
    pub recipient: Recipient,
}

/// Test email input.
#[derive(Serialize, Deserialize)]
pub struct TestRequest {
    #[serde(default)]
    /// Selected email template.
    pub template: EmailTemplate,
    #[serde(default)]
    /// Selected landing page.
    pub page: LandingPage,
    /// Explicit sending-profile selection.
    pub smtp: super::TestProfile,
    #[serde(default)]
    /// Configured destination URL.
    pub url: String,
    #[serde(default)]
    /// Recipient email address.
    pub email: String,
    #[serde(default)]
    /// Recipient given name.
    pub first_name: String,
    #[serde(default)]
    /// Recipient family name.
    pub last_name: String,
    #[serde(default)]
    /// Recipient position.
    pub position: String,
}

/// Authored static learning-page input; public identity is always server owned.
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct LearningPage {
    /// Existing resource ID; ignored when creating.
    #[serde(default)]
    pub id: i64,
    /// Shared resource name.
    pub name: String,
    /// Static authored HTML.
    pub html: String,
}

/// Writable resource selected by its endpoint, never inferred from overlapping fields.
#[derive(Clone, Serialize)]
#[serde(untagged)]
pub enum Resource {
    /// Recipient-group write.
    Group(Group),
    /// Email-template write.
    Template(EmailTemplate),
    /// Landing-page write.
    Page(LandingPage),
    /// Static learning-page write.
    LearningPage(LearningPage),
    /// Sending-profile write with an optional replacement credential.
    Profile(SendingProfile),
}
/// Project JSON draft/import fields into the authoritative writable contract.
/// Unknown response metadata is intentionally discarded; it can never be submitted by this projection.
pub fn decode(
    kind: crate::resource_details::Kind,
    value: serde_json::Value,
) -> Result<Resource, serde_json::Error> {
    use crate::resource_details::Kind;
    match kind {
        Kind::Group => serde_json::from_value(value).map(Resource::Group),
        Kind::Template => serde_json::from_value(value).map(Resource::Template),
        Kind::Page => serde_json::from_value(value).map(Resource::Page),
        Kind::LearningPage => serde_json::from_value(value).map(Resource::LearningPage),
        Kind::Profile => serde_json::from_value(value).map(Resource::Profile),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_details::Kind;
    use serde_json::{Value, json};

    #[test]
    fn resource_inputs_project_only_writable_fields_including_nested_records() {
        let cases = [
            (
                Kind::Group,
                json!({"name":"Audience","targets":[{"id":99,"user_id":99,"email":"person@example.test"}]}),
            ),
            (
                Kind::Template,
                json!({"name":"Mail","html":"<p>Hi</p>","attachments":[{"id":99,"name":"note","type":"text/plain","content":"SGk="}]}),
            ),
            (Kind::Page, json!({"name":"Page","html":"<p>Hi</p>"})),
            (
                Kind::LearningPage,
                json!({"name":"Lesson","html":"<p>Hi</p>"}),
            ),
            (
                Kind::Profile,
                json!({"name":"SMTP","host":"relay.example.test","headers":[{"id":99,"key":"X-Test","value":"fixture"}]}),
            ),
        ];
        for (kind, mut fields) in cases {
            for key in [
                "user_id",
                "owner_id",
                "revision",
                "modified_date",
                "created_at",
                "created_by",
                "edited_by",
                "edited_at",
                "public_id",
                "warnings",
                "preview_html",
                "has_password",
            ] {
                fields[key] = json!({"malformed":"untrusted metadata"});
            }
            let projected = serde_json::to_value(decode(kind, fields).unwrap()).unwrap();
            for key in [
                "user_id",
                "owner_id",
                "revision",
                "modified_date",
                "created_at",
                "created_by",
                "edited_by",
                "edited_at",
                "public_id",
                "warnings",
                "preview_html",
                "has_password",
            ] {
                assert!(
                    projected.get(key).is_none(),
                    "{key} must never enter a request"
                );
            }
            for key in ["targets", "attachments", "headers"] {
                for child in projected[key].as_array().into_iter().flatten() {
                    assert!(child.get("id").is_none());
                    assert!(child.get("user_id").is_none());
                }
            }
        }
    }
    #[test]
    fn profile_password_omission_clear_and_replacement_survive_projection() {
        for (field, expected) in [
            (None, None),
            (Some(Value::Null), None),
            (Some(json!("")), Some("")),
            (Some(json!("replacement")), Some("replacement")),
        ] {
            let mut value = json!({"name":"SMTP"});
            if let Some(field) = field {
                value["password"] = field;
            }
            let Resource::Profile(profile) = decode(Kind::Profile, value).unwrap() else {
                panic!("wrong kind")
            };
            assert_eq!(profile.password.as_deref(), expected);
            let wire = serde_json::to_value(profile).unwrap();
            assert_eq!(wire.get("password").and_then(Value::as_str), expected);
        }
    }
}
