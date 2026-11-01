//! Read-only authorship within the shared installation, independent of authorization.

/// Stable account identifiers survive account deletion and are never reassigned.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Attribution {
    /// Original creator, or unknown for historical records without reliable attribution.
    pub created_by: Option<i64>,
    /// Last authenticated editor; background processing does not change this value.
    pub edited_by: Option<i64>,
    /// UTC instant of the last successful human edit; unknown for imported history.
    pub edited_at: Option<String>,
}
