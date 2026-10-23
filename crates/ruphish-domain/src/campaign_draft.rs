//! Incomplete campaign configuration. Saving never authorizes execution.
use serde::{Deserialize, Serialize};

/// A saved resource reference, never a resource body or service credential.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Reference {
    /// Zero represents an unselected resource.
    pub id: i64,
}

/// Ordered audience and content selections.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Selections {
    /// Ordered groups, retaining audience precedence.
    pub groups: Vec<Reference>,
    /// Email template.
    pub template: Reference,
    /// Landing page.
    pub page: Reference,
    /// Sending profile.
    pub smtp: Reference,
}

/// Editable local schedule, including incomplete input.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LocalTime {
    /// Civil date and time.
    pub local: String,
    /// IANA zone used when reviewing and launching.
    pub time_zone: String,
}

/// Presence-only interaction settings. No participant values may be stored.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    /// Inert legacy draft input; submissions are always tracked.
    pub capture_credentials: bool,
    /// Inert legacy draft input; presence is always tracked.
    pub capture_passwords: bool,
    /// Optional redirect expression.
    pub redirect_url: String,
}

/// Versioned, closed configuration accepted by draft persistence.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    /// Campaign display name; empty is permitted until launch.
    pub name: String,
    /// Public destination; empty is permitted until launch.
    pub url: String,
    /// Resource references only.
    pub selection: Selections,
    /// None means page defaults have not been resolved yet.
    pub policy: Option<Policy>,
    /// None requests immediate delivery at launch.
    pub launch_local: Option<LocalTime>,
    /// Optional end of initial distribution.
    pub send_by_local: Option<LocalTime>,
}
impl Configuration {
    /// Bound saved input without requiring a launchable campaign.
    pub fn validate(&self) -> bool {
        let text = |value: &str, max| value.chars().count() <= max && !value.contains('\0');
        text(&self.name, 255)
            && text(&self.url, 8192)
            && self.selection.groups.len() <= 1000
            && self
                .selection
                .groups
                .iter()
                .chain([
                    &self.selection.template,
                    &self.selection.page,
                    &self.selection.smtp,
                ])
                .all(|reference| reference.id >= 0)
            && self
                .policy
                .as_ref()
                .is_none_or(|policy| text(&policy.redirect_url, 8192))
            && [&self.launch_local, &self.send_by_local]
                .into_iter()
                .flatten()
                .all(|time| text(&time.local, 100) && text(&time.time_zone, 100))
    }
}
