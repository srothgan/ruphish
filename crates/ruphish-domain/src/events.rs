//! Canonical observation kinds. Legacy labels belong to compatibility projections.

/// One observed action; repeats are distinct events, not state transitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Campaign creation.
    CampaignCreated,
    /// SMTP acceptance.
    EmailSent,
    /// SMTP delivery failure.
    EmailError,
    /// Tracking image request, not proof of a human opening a message.
    EmailOpened,
    /// Landing request, not proof of a human clicking.
    LinkClicked,
    /// Form submission, with optional password-presence evidence.
    FormSubmitted,
    /// Public or mailbox report.
    EmailReported,
    /// Administrator delivery reconciliation.
    DeliveryReconciled,
}

impl Kind {
    /// All recognized observations in presentation order.
    pub const ALL: [Self; 8] = [
        Self::CampaignCreated,
        Self::EmailSent,
        Self::EmailError,
        Self::EmailOpened,
        Self::LinkClicked,
        Self::FormSubmitted,
        Self::EmailReported,
        Self::DeliveryReconciled,
    ];

    /// Decode a stable event identifier at an input or storage boundary.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key)
    }

    /// Stable storage identifier.
    pub const fn key(self) -> &'static str {
        match self {
            Self::CampaignCreated => "campaign_created",
            Self::EmailSent => "email_sent",
            Self::EmailError => "email_error",
            Self::EmailOpened => "email_opened",
            Self::LinkClicked => "link_clicked",
            Self::FormSubmitted => "form_submitted",
            Self::EmailReported => "email_reported",
            Self::DeliveryReconciled => "delivery_reconciled",
        }
    }

    /// Preserved public label; never use a label as a recipient's event identity.
    pub const fn label(self) -> &'static str {
        match self {
            Self::CampaignCreated => "Campaign Created",
            Self::EmailSent => "Email Sent",
            Self::EmailError => "Error Sending Email",
            Self::EmailOpened => "Email Opened",
            Self::LinkClicked => "Clicked Link",
            Self::FormSubmitted => "Submitted Data",
            Self::EmailReported => "Email Reported",
            Self::DeliveryReconciled => "Delivery reconciled",
        }
    }

    /// Decode an existing public label at a compatibility boundary.
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "Campaign Created" => Some(Self::CampaignCreated),
            "Email Sent" => Some(Self::EmailSent),
            "Error Sending Email" => Some(Self::EmailError),
            "Email Opened" => Some(Self::EmailOpened),
            "Clicked Link" => Some(Self::LinkClicked),
            "Submitted Data" => Some(Self::FormSubmitted),
            "Email Reported" => Some(Self::EmailReported),
            "Delivery reconciled" => Some(Self::DeliveryReconciled),
            _ => None,
        }
    }
}

/// A classified observation or an unrecognized name retained from imported history.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum RecordedKind {
    /// One of the canonical observation kinds.
    Known(Kind),
    /// Preserved imported name; never treated as evidence for a known action.
    Historical(String),
}
impl RecordedKind {
    /// Classify a stored event-history label without discarding unknown history.
    pub fn from_label(label: String) -> Self {
        Kind::from_label(&label).map_or_else(|| Self::Historical(label), Self::Known)
    }
    /// Human-readable event name for presentation and tabular exports.
    pub fn label(&self) -> &str {
        match self {
            Self::Known(kind) => kind.label(),
            Self::Historical(label) => label,
        }
    }
}

/// Closed event metadata. No field values or value derivatives can be represented.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Metadata {
    /// Explicit historical presence; never inferred from submitted payloads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username_present: Option<bool>,
    /// Presence of an eligible nonempty password control at submission.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_present: Option<bool>,
    /// Classified delivery outcome.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<DeliveryCode>,
    /// Explicit administrator reconciliation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<Resolution>,
    /// Administrator identity, unrelated to participant input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<u64>,
}
/// Fixed codes, never SMTP peer text or database diagnostics.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryCode {
    /// Ambiguous external acceptance.
    DeliveryOutcomeUnknown,
    /// Permanent SMTP refusal.
    SmtpPermanentRejection,
    /// Operator action required.
    SmtpProfileBlocked,
    /// Temporary SMTP refusal.
    SmtpTemporaryRejection,
    /// No retries remain.
    DeliveryRetryExhausted,
}
/// Supported administrator reconciliation actions.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// Retry a known failure.
    Retry,
    /// Explicitly resend an ambiguous delivery.
    ResendUnknown,
    /// Confirm external acceptance.
    ConfirmAccepted,
}
