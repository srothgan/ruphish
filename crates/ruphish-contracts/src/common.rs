//! Shared response envelopes, timestamps, and JSON encoding for the public API.

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
/// Expected revision for an explicit management action.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revision {
    /// Required saved revision reviewed by the caller.
    pub revision: i64,
}
