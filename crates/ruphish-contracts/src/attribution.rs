//! Self-contained, read-only attribution on resource responses.

/// A safe account reference, never an authentication or account-management record.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UserReference {
    /// Stable account identifier, retained after deletion.
    pub id: i64,
    /// Display name, or unknown when historical identity information is unavailable.
    #[serde(deserialize_with = "crate::nullable")]
    pub username: Option<String>,
    /// True when a retained deleted-account record confirms deletion.
    pub deleted: bool,
}

/// Attribution is returned with the resource and cannot be assigned by a client.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Attribution {
    /// UTC creation instant, or unknown when historical evidence is unavailable.
    #[serde(deserialize_with = "crate::nullable")]
    pub created_at: Option<String>,
    /// Original creator, or unknown for unattributed historical resources.
    #[serde(deserialize_with = "crate::nullable")]
    pub created_by: Option<UserReference>,
    /// Last authenticated editor, or unknown for imported history.
    #[serde(deserialize_with = "crate::nullable")]
    pub edited_by: Option<UserReference>,
    /// UTC timestamp of the successful save.
    #[serde(deserialize_with = "crate::nullable")]
    pub edited_at: Option<String>,
}
