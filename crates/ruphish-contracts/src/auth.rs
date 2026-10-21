//! Explicit authentication wire types. Password hashes and session secrets never belong here.
use crate::legacy::Timestamp;
use serde::{Deserialize, Serialize};

/// Legacy public role metadata.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    /// Stable role name.
    pub slug: String,
    /// Display name.
    pub name: String,
    /// Description.
    pub description: String,
}
/// Authorized legacy user response, including the existing API credential contract.
#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    /// Persistent identifier.
    pub id: i64,
    /// Exact login name.
    pub username: String,
    /// Automation credential; returned only by authorized account endpoints.
    pub api_key: String,
    /// Role metadata.
    pub role: Role,
    /// Mandatory password setup.
    pub password_change_required: bool,
    /// Account is disabled.
    pub account_locked: bool,
    /// Most recent successful login.
    pub last_login: Timestamp,
}
/// Full legacy create/update input; an empty update password preserves its hash.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UserEdit {
    /// Exact login name.
    pub username: String,
    /// New password, if provided.
    pub password: String,
    /// Role slug.
    pub role: String,
    /// Require password setup.
    pub password_change_required: bool,
    /// Disable authentication.
    pub account_locked: bool,
}
/// Browser bootstrap deliberately excludes API credentials.
#[derive(Clone, Serialize, Deserialize)]
pub struct BrowserSession {
    /// Account display overrides; null fields follow this browser's defaults.
    #[serde(default)]
    pub preferences: Preferences,
    /// Token to submit with unsafe cookie-authenticated requests.
    pub csrf_token: String,
    /// Signed-in account, or none.
    pub user: Option<BrowserUser>,
    /// Original administrator while impersonating.
    pub actor: Option<String>,
}

/// Complete display preference replacement. Null clears an override.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    /// Supported interface language, or browser default.
    pub language: Option<String>,
    /// BCP-47 date/time formatting locale, or browser default.
    pub format_locale: Option<String>,
    /// IANA time-zone name, or browser default.
    pub time_zone: Option<String>,
    /// Whether date labels include their UTC offset.
    #[serde(default = "default_show_utc_offset")]
    pub show_utc_offset: bool,
}
/// Minimal browser identity and current capabilities.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserUser {
    /// Persistent identifier.
    pub id: i64,
    /// Display/login name.
    pub username: String,
    /// Current role.
    pub role: Role,
    /// Complete setup before other operations.
    pub password_change_required: bool,
    /// Current persisted permissions for presentation; server remains authoritative.
    pub permissions: Vec<String>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: None,
            format_locale: None,
            time_zone: None,
            show_utc_offset: true,
        }
    }
}

fn default_show_utc_offset() -> bool {
    true
}
