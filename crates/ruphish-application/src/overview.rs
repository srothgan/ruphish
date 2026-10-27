//! Owner-scoped dashboard aggregate: one authorized read-model spanning campaigns,
//! engagement, resources and domains. Read-only, with no mutation, event payloads
//! or entered participant values.
use crate::{auth::Identity, resources::Error};

/// Rolling window for the first recorded SMTP acceptance of each campaign recipient.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Window {
    /// All recorded engagement, regardless of age.
    All,
    /// The trailing seven days.
    Last7Days,
    /// The trailing thirty days.
    Last30Days,
    /// The trailing 365 days, not the current calendar year.
    LastYear,
}
impl Window {
    /// Inclusive lower bound in UTC Unix seconds, or `None` for all time.
    pub fn since(self, now: i64) -> Option<i64> {
        match self {
            Self::All => None,
            Self::Last7Days => Some(now - 7 * 86_400),
            Self::Last30Days => Some(now - 30 * 86_400),
            Self::LastYear => Some(now - 365 * 86_400),
        }
    }
    /// Stable wire token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Last7Days => "7d",
            Self::Last30Days => "30d",
            Self::LastYear => "365d",
        }
    }
    /// Parse a wire token, rejecting anything unrecognized.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "all" => Some(Self::All),
            "7d" => Some(Self::Last7Days),
            "30d" => Some(Self::Last30Days),
            "365d" => Some(Self::LastYear),
            _ => None,
        }
    }
}

/// Engagement of uniquely identified campaign recipients first accepted within
/// the selected window, observed through the server snapshot time. Repeated
/// events count once; opens include clicks/submissions and clicks include
/// submissions. Credentials entered counts only explicit boolean presence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Posture {
    /// Recipients a message was recorded as sent to.
    pub delivered: i64,
    /// Recipients with an observed open, click or submission.
    pub opened: i64,
    /// Recipients with an observed click or submission.
    pub clicked: i64,
    /// Recipients who reported a message.
    pub reported: i64,
    /// Recipients whose submission explicitly recorded password presence.
    pub credentials_entered: i64,
}

/// Counts for the preceding, non-overlapping acceptance period.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviousPeriod {
    /// Inclusive acceptance-time lower bound.
    pub since: i64,
    /// Exclusive upper bound for acceptance and observations.
    pub until: i64,
    /// Outcomes observed before this period ended.
    pub counts: Posture,
}

/// Current metrics and an optional comparison from the same database snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Metrics {
    /// Selected period outcomes.
    pub counts: Posture,
    /// No preceding period exists for all time.
    pub previous: Option<PreviousPeriod>,
}

/// A recent campaign row for the dashboard list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentCampaign {
    /// Campaign identity.
    pub id: i64,
    /// Display name.
    pub name: String,
    /// Lifecycle state.
    pub status: String,
    /// Scheduled launch as stored (ISO-8601), or empty when unset.
    pub launch: String,
    /// Audience size.
    pub recipients: i64,
    /// SMTP-accepted recipients and cumulative progression for this campaign.
    pub outcomes: Posture,
}

/// Resource inventory counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    /// Email templates.
    pub templates: i64,
    /// Landing pages.
    pub pages: i64,
    /// Recipient groups.
    pub groups: i64,
    /// Sending profiles.
    pub profiles: i64,
    /// Configured domains.
    pub domains: i64,
    /// Domains currently verified.
    pub domains_verified: i64,
    /// Reporting mailbox resources.
    pub reporting_mailboxes: i64,
}

/// A campaign scheduled to launch in the future.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextLaunch {
    /// Campaign identity.
    pub id: i64,
    /// Display name.
    pub name: String,
    /// Scheduled launch as stored (ISO-8601).
    pub launch: String,
}

/// Items an operator should act on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attention {
    /// Enabled domains that are not currently verified.
    pub unverified_enabled_domains: i64,
    /// Current failed, blocked or uncertain deliveries in unfinished campaigns.
    pub delivery_problems: i64,
    /// The soonest future scheduled launch, if any.
    pub next_launch: Option<NextLaunch>,
}

/// Everything the dashboard needs except the windowed posture.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// Server time used consistently for expiry and launch calculations.
    pub as_of: i64,
    /// Most recent campaigns, newest first.
    pub recent: Vec<RecentCampaign>,
    /// Resource inventory.
    pub inventory: Inventory,
    /// Attention items.
    pub attention: Attention,
}

#[async_trait::async_trait]
/// Read-only dashboard aggregation, authorized to one owner.
pub trait Repository: Send + Sync {
    /// One consistent snapshot of campaigns, inventory and attention items.
    async fn snapshot(&self, caller: Identity, now: i64) -> Result<Snapshot, Error>;
    /// Windowed engagement across the owner's campaigns.
    async fn posture(&self, caller: Identity, window: Window, now: i64) -> Result<Metrics, Error>;
}
