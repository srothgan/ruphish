//! Compatibility-sensitive JSON values used by the existing HTTP API.

use ruphish_domain::datetime;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Existing success/error envelope; absent payloads serialize as explicit null.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Envelope<T> {
    /// Whether the operation succeeded.
    pub success: bool,
    /// Existing public message.
    pub message: String,
    /// Optional payload, always present on the wire.
    pub data: Option<T>,
}

/// An explicit RFC3339 instant, normalized to UTC on output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timestamp(pub datetime::Timestamp);

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        datetime::utc(self.0).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        datetime::parse(&value)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

/// A recipient's public fields, without persistence identifiers or ownership.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
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
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Group {
    /// Public group ID.
    pub id: i64,
    /// Group name.
    pub name: String,
    /// Last modification time.
    pub modified_date: Option<Timestamp>,
    /// Recipient records; an empty collection is an empty array.
    pub targets: Vec<Recipient>,
}

/// Custom SMTP header without internal IDs.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Header {
    /// Header name.
    pub key: String,
    /// Header value.
    pub value: String,
}

/// Existing sending-profile wire fields; empty credentials are omitted.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
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
    /// SMTP login credential, exposed by the existing authorized API contract.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub password: String,
    /// Sender mailbox.
    pub from_address: String,
    /// Existing certificate-verification setting.
    pub ignore_cert_errors: bool,
    /// Custom headers; empty when none are configured.
    pub headers: Vec<Header>,
    /// Modification time.
    pub modified_date: Option<Timestamp>,
}

/// Attachment bytes are base64 on the existing API, without persistence IDs.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct Attachment {
    /// Base64 content.
    pub content: String,
    /// MIME type.
    #[serde(rename = "type")]
    pub mime_type: String,
    /// Display filename.
    pub name: String,
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
    /// Existing modification timestamp.
    pub modified_date: Option<Timestamp>,
    /// Attachments; empty when none are configured.
    pub attachments: Vec<Attachment>,
}

/// Landing-page wire contract.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LandingPage {
    /// Public identifier.
    pub id: i64,
    /// Owner-scoped display name.
    pub name: String,
    /// Authored document source.
    pub html: String,
    /// Templated destination after submission.
    pub redirect_url: String,
    /// Last persisted change.
    pub modified_date: Option<Timestamp>,
}

/// Serialize JSON using Go's HTML/line-separator escaping convention.
pub fn json<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let value = serde_json::to_string_pretty(value)?;
    Ok(value
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
        .into_bytes())
}
