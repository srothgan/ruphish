//! Dashboard response contracts. Counts contain no participant identities or submitted values.
/// Engagement of uniquely identified campaign recipients first accepted within
/// the selected window, observed through the server snapshot time. Repeated
/// events count once. Credentials entered counts only explicit boolean presence.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Posture {
    /// Recipients a message was recorded as sent to.
    pub delivered: i64,
    /// Recipients who opened a message.
    pub opened: i64,
    /// Recipients who followed a link.
    pub clicked: i64,
    /// Recipients who reported a message.
    pub reported: i64,
    /// Recipients whose submission explicitly recorded password presence.
    pub credentials_entered: i64,
}

/// Campaign lifecycle counts for the owner.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CampaignCounts {
    /// All campaigns the owner holds.
    pub total: i64,
    /// Campaigns currently sending or in progress.
    pub active: i64,
    /// Campaigns scheduled to start later.
    pub scheduled: i64,
    /// Campaigns that have finished.
    pub completed: i64,
}

/// A recent campaign row for the dashboard list.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
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
    /// SMTP-accepted recipients and observed interactions for this campaign.
    pub outcomes: Posture,
}

/// Resource inventory counts.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
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
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NextLaunch {
    /// Campaign identity.
    pub id: i64,
    /// Display name.
    pub name: String,
    /// Scheduled launch as stored (ISO-8601).
    pub launch: String,
}

/// Items an operator should act on.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Attention {
    /// Enabled domains that are not currently verified.
    pub unverified_enabled_domains: i64,
    /// Current failed, blocked or uncertain deliveries in unfinished campaigns.
    pub delivery_problems: i64,
    /// The soonest future scheduled launch, if any.
    pub next_launch: Option<NextLaunch>,
}

/// Everything the dashboard needs except the windowed posture.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// Server time used consistently for expiry and launch calculations.
    pub as_of: i64,
    /// Lifecycle counts.
    pub campaigns: CampaignCounts,
    /// Most recent campaigns, newest first.
    pub recent: Vec<RecentCampaign>,
    /// Resource inventory.
    pub inventory: Inventory,
    /// Attention items.
    pub attention: Attention,
}

/// Selected acceptance cohort and its observation time.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PostureResponse {
    /// all, 7d, 30d or 365d.
    pub window: String,
    /// Inclusive acceptance-time lower bound; absent for all time.
    pub since: Option<i64>,
    /// Inclusive server observation time in Unix seconds.
    pub as_of: i64,
    /// Distinct campaign-recipient outcomes.
    #[serde(flatten)]
    pub counts: Posture,
    /// Preceding equal-length period, absent for all time.
    pub previous: Option<PreviousPeriod>,
}

/// Preceding period used for absolute count changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PreviousPeriod {
    /// Inclusive acceptance-time lower bound.
    pub since: i64,
    /// Exclusive upper bound for acceptance and observations.
    pub until: i64,
    /// Interaction counts only; send-volume comparisons are not exposed.
    pub counts: InteractionCounts,
}

/// Previous-period interactions used for count changes, without a send-volume metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InteractionCounts {
    /// Recipients with a recorded open.
    pub opened: i64,
    /// Recipients with a recorded click.
    pub clicked: i64,
    /// Recipients with a recorded report.
    pub reported: i64,
    /// Recipients with explicitly recorded password presence.
    pub credentials_entered: i64,
}
