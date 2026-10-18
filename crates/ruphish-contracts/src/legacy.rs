//! Compatibility-sensitive JSON values used by the existing HTTP API.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

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

/// An RFC3339 timestamp with Go's zero value and trimmed fractional seconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timestamp(pub OffsetDateTime);

impl Default for Timestamp {
    fn default() -> Self {
        Self(time::macros::datetime!(0001-01-01 00:00 UTC))
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0
            .format(&Rfc3339)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(value) = Option::<String>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        OffsetDateTime::parse(&value, &Rfc3339)
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

/// A group response with a deliberate distinction between null and empty targets.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Group {
    /// Public group ID.
    pub id: i64,
    /// Group name.
    pub name: String,
    /// Last modification time.
    pub modified_date: Timestamp,
    /// None preserves a nil Go slice; Some(empty) preserves an empty slice.
    pub targets: Option<Vec<Recipient>>,
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
    /// Null or populated custom headers.
    pub headers: Option<Vec<Header>>,
    /// Modification time.
    pub modified_date: Timestamp,
}

/// The string-encoded integer convention used by IMAP JSON fields.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct QuotedNumber<T>(pub T);

impl<T: Default + PartialEq> QuotedNumber<T> {
    /// Whether Go's omitempty excludes this value.
    pub fn is_zero(&self) -> bool {
        self.0 == T::default()
    }
}
impl<T: std::fmt::Display> Serialize for QuotedNumber<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}
impl<'de, T> Deserialize<'de> for QuotedNumber<T>
where
    T: std::str::FromStr + Default,
    T::Err: std::fmt::Display,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(value) = Option::<String>::deserialize(deserializer)? else {
            return Ok(Self(T::default()));
        };
        if value == "null" {
            return Ok(Self(T::default()));
        }
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(serde::de::Error::custom("expected unsigned integer string"));
        }
        value.parse().map(Self).map_err(serde::de::Error::custom)
    }
}

/// Decode one JSON object, preserving Go's null-as-zero struct behavior.
///
/// This boundary rejects arrays even though Serde's struct sequence representation
/// would otherwise accept an empty array. The caller owns domain validation.
pub fn decode_object<T: serde::de::DeserializeOwned + Default>(
    bytes: &[u8],
) -> Result<T, serde_json::Error> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) == Some(&b'{') {
        // Decode the input map directly: collecting into Value first would lose
        // duplicate-field ordering and Go's null-does-not-overwrite semantics.
        return T::deserialize(&mut decoder);
    }
    let value = serde_json::Value::deserialize(&mut decoder)?;
    if value.is_null() {
        return Ok(T::default());
    }
    if !value.is_object() {
        return Err(serde::de::Error::custom("expected JSON object"));
    }
    Err(serde::de::Error::custom("expected JSON object"))
}

/// IMAP settings retain quoted integers and zero timestamp fields.
#[derive(Debug, Default, Serialize, PartialEq)]
pub struct Imap {
    /// Whether polling is enabled.
    pub enabled: bool,
    /// Server name.
    pub host: String,
    /// String-encoded port, omitted when zero.
    #[serde(skip_serializing_if = "QuotedNumber::is_zero")]
    pub port: QuotedNumber<u16>,
    /// Login name.
    pub username: String,
    /// Existing authorized API credential field.
    pub password: String,
    /// TLS setting.
    pub tls: bool,
    /// Certificate setting.
    pub ignore_cert_errors: bool,
    /// Mailbox name.
    pub folder: String,
    /// Restriction domain.
    pub restrict_domain: String,
    /// Whether reported messages are deleted.
    pub delete_reported_campaign_email: bool,
    /// Zero time remains present despite Go's omitempty tag on time.Time.
    pub last_login: Timestamp,
    /// Modification time.
    pub modified_date: Timestamp,
    /// String-encoded polling frequency.
    #[serde(skip_serializing_if = "QuotedNumber::is_zero")]
    pub imap_freq: QuotedNumber<u32>,
}

impl<'de> Deserialize<'de> for Imap {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct Fields;
        impl<'de> serde::de::Visitor<'de> for Fields {
            type Value = Imap;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an IMAP settings object")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(self, mut map: M) -> Result<Imap, M::Error> {
                let mut value = Imap::default();
                while let Some(name) = map.next_key::<String>()? {
                    // Go uses case folding, including long-s for ASCII S.
                    let name: String = name
                        .chars()
                        .flat_map(char::to_lowercase)
                        .map(|character| if character == 'ſ' { 's' } else { character })
                        .collect();
                    macro_rules! field {
                        ($field:ident, $ty:ty) => {
                            if let Some(next) = map.next_value::<Option<$ty>>()? {
                                value.$field = next;
                            }
                        };
                    }
                    macro_rules! quoted_field {
                        ($field:ident, $ty:ty) => {{
                            let next = map.next_value::<serde_json::Value>()?;
                            if !next.is_null() && next != "null" {
                                value.$field = serde_json::from_value::<QuotedNumber<$ty>>(next)
                                    .map_err(serde::de::Error::custom)?;
                            }
                        }};
                    }
                    match name.as_str() {
                        "enabled" => {
                            field!(enabled, bool);
                        }
                        "host" => {
                            field!(host, String);
                        }
                        "port" => {
                            quoted_field!(port, u16);
                        }
                        "username" => {
                            field!(username, String);
                        }
                        "password" => {
                            field!(password, String);
                        }
                        "tls" => {
                            field!(tls, bool);
                        }
                        "ignore_cert_errors" => {
                            field!(ignore_cert_errors, bool);
                        }
                        "folder" => {
                            field!(folder, String);
                        }
                        "restrict_domain" => {
                            field!(restrict_domain, String);
                        }
                        "delete_reported_campaign_email" => {
                            field!(delete_reported_campaign_email, bool);
                        }
                        "last_login" => {
                            field!(last_login, Timestamp);
                        }
                        "modified_date" => {
                            field!(modified_date, Timestamp);
                        }
                        "imap_freq" => {
                            quoted_field!(imap_freq, u32);
                        }
                        _ => {
                            let _ = map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(value)
            }
        }
        decoder.deserialize_map(Fields)
    }
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
    pub modified_date: Timestamp,
    /// None preserves the existing nil-slice wire representation.
    pub attachments: Option<Vec<Attachment>>,
}

/// Persisted landing-page wire contract, with original capture flags.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LandingPage {
    /// Public identifier.
    pub id: i64,
    /// Owner-scoped display name.
    pub name: String,
    /// Authored document source.
    pub html: String,
    /// Record submitted fields.
    pub capture_credentials: bool,
    /// Include passwords; implies capture_credentials.
    pub capture_passwords: bool,
    /// Templated destination after submission.
    pub redirect_url: String,
    /// Last persisted change.
    pub modified_date: Timestamp,
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
