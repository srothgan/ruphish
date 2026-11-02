//! Wire values with identical request and response semantics.
use serde::{Deserialize, Serialize};

pub use ruphish_domain::resources::Header;

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
