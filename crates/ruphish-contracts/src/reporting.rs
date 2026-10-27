//! Browser-safe reporting requests. No connection credentials or participant payloads.
use serde::{Deserialize, Serialize};
/// URL filters shared by analysis and download links.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Filter {
    /// Recipient row identity.
    pub recipient: Option<i64>,
    /// Comma-separated recipient identities for multi-selection; empty means all.
    pub recipients: String,
    /// Inclusive UTC seconds.
    pub from: Option<i64>,
    /// Exclusive UTC seconds.
    pub until: Option<i64>,
    /// Event identifier supplied by the client; the HTTP boundary validates it.
    pub event: String,
    /// Recipient search.
    pub search: String,
    /// Observation source supplied by the client; the HTTP boundary validates it.
    pub source: String,
    /// Event ordering before pagination.
    pub event_sort: ruphish_domain::reporting::EventSort,
    /// Event direction, independent of recipient ordering.
    pub event_order: ruphish_domain::reporting::SortDirection,
    /// Recipient ordering before pagination, independent of event ordering.
    pub sort: ruphish_domain::reporting::RecipientSort,
    /// Sort direction; equal values always use ascending recipient identity.
    pub order: ruphish_domain::reporting::SortDirection,
    /// Row offset.
    pub offset: usize,
    /// Page length.
    pub limit: usize,
    /// Download format.
    pub format: String,
    /// Download scope.
    pub scope: String,
}
impl Default for Filter {
    fn default() -> Self {
        Self {
            recipient: None,
            recipients: String::new(),
            from: None,
            until: None,
            event: String::new(),
            search: String::new(),
            source: String::new(),
            event_sort: Default::default(),
            event_order: ruphish_domain::reporting::SortDirection::Desc,
            sort: Default::default(),
            order: Default::default(),
            offset: 0,
            limit: 50,
            format: "csv".into(),
            scope: "events".into(),
        }
    }
}

/// Named observation metrics. The enclosing field defines whether these are raw
/// totals, unique observed recipients or cumulative recipient progression.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Counts {
    /// Recorded SMTP acceptances; never inferred from engagement.
    pub accepted: i64,
    /// Image fetches, or cumulative open progression in a progression field.
    pub opened: i64,
    /// Landing visits, or cumulative click progression in a progression field.
    pub clicked: i64,
    /// Form submissions, independent of entered-value contents.
    pub submitted: i64,
    /// Reports, independent of interaction progression.
    pub reported: i64,
}
impl From<[i64; 5]> for Counts {
    fn from([accepted, opened, clicked, submitted, reported]: [i64; 5]) -> Self {
        Self {
            accepted,
            opened,
            clicked,
            submitted,
            reported,
        }
    }
}
impl Counts {
    /// Presentation order for the five chart series or table columns.
    pub const fn values(self) -> [i64; 5] {
        [
            self.accepted,
            self.opened,
            self.clicked,
            self.submitted,
            self.reported,
        ]
    }
}
