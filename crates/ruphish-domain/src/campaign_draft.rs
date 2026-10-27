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

/// Post-submission destination settings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    /// Optional redirect expression.
    pub redirect_url: String,
    /// Selected owned learning page; custom redirect must be empty when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learning_page_id: Option<i64>,
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
            && self.policy.as_ref().is_none_or(|policy| {
                text(&policy.redirect_url, 8192)
                    && policy
                        .learning_page_id
                        .is_none_or(|id| id >= 0 && policy.redirect_url.is_empty())
            })
            && [&self.launch_local, &self.send_by_local]
                .into_iter()
                .flatten()
                .all(|time| text(&time.local, 100) && text(&time.time_zone, 100))
    }
}

impl Configuration {
    /// Readiness through a wizard step. Persistence deliberately uses `validate` instead.
    pub fn readiness(&self, through: usize, now: i64) -> crate::validation::Report {
        use crate::validation::{Code, Report};
        let mut fields = crate::campaign::Fields {
            name: &self.name,
            groups: !self.selection.groups.is_empty()
                && self.selection.groups.iter().all(|r| r.id > 0),
            template: self.selection.template.id > 0,
            page: self.selection.page.id > 0,
            smtp: self.selection.smtp.id > 0,
            launch: now,
            send_by: None,
        };
        let mut report = fields.audience_validation();
        if through >= 1 {
            report.extend(fields.content_validation());
            if self.url.is_empty() {
                report.reject("url", Code::Required, "Campaign destination not specified");
            } else if crate::domains::origin(&self.url, true).is_err() {
                report.reject("url", Code::Invalid, "Invalid campaign destination");
            }
            match &self.policy {
                None => report.reject(
                    "selection.page.id",
                    Code::Unavailable,
                    "Draft policy is incomplete",
                ),
                Some(policy) => report.extend(crate::campaign::policy_validation(
                    policy.learning_page_id,
                    &policy.redirect_url,
                )),
            }
        }
        if through >= 2 {
            let mut times = Report::default();
            let mut resolve = |time: &Option<LocalTime>, field: &str| -> Option<i64> {
                let time = time.as_ref()?;
                if time.local.is_empty() {
                    times.reject(
                        format!("{field}.local"),
                        Code::Required,
                        "Choose a date and time",
                    );
                    return None;
                }
                match crate::datetime::resolve_local(&time.local, &time.time_zone) {
                    Ok(value) => Some(value.as_second()),
                    Err(crate::datetime::Error::UnknownZone) => {
                        times.reject(
                            format!("{field}.time_zone"),
                            Code::Invalid,
                            "Unknown time zone",
                        );
                        None
                    }
                    Err(_) => {
                        times.reject(
                            format!("{field}.local"),
                            Code::Invalid,
                            "Invalid local date/time",
                        );
                        None
                    }
                }
            };
            let launch = resolve(&self.launch_local, "launch_local");
            let end = resolve(&self.send_by_local, "send_by_local");
            if launch.is_some_and(|time| time < now) {
                times.reject(
                    "launch_local.local",
                    Code::SchedulePast,
                    "Draft schedule is in the past",
                );
            }
            if times.is_empty() {
                fields.launch = launch.unwrap_or(now);
                fields.send_by = end;
                times.extend(fields.schedule_validation());
            }
            report.extend(times);
        }
        report
    }
}

/// The editing step which owns a diagnostic, independent of browser markup.
pub fn field_step(field: &str) -> usize {
    if field == "name" || field.starts_with("selection.groups") {
        0
    } else if field.starts_with("launch_local") || field.starts_with("send_by_local") {
        2
    } else {
        1
    }
}

/// Draft launch acknowledgement, derived from its committed campaign reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Editable configuration without an execution.
    Draft,
    /// Launch committed; the draft remains available for retry reconciliation.
    Launched,
}
impl Status {
    /// The committed reference is the sole authority; no separate state is persisted.
    pub const fn from_campaign_id(campaign_id: Option<i64>) -> Self {
        if campaign_id.is_some() {
            Self::Launched
        } else {
            Self::Draft
        }
    }
}
