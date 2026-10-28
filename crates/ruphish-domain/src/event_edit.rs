//! Corrections target one observation; they never lock a recipient outcome.
use crate::{
    datetime,
    events::{Kind, RecordedKind},
};
use serde::{Deserialize, Serialize};

/// Closed participant-presence information, without values or derivatives.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presence {
    /// Explicit username presence, if known.
    pub username_present: Option<bool>,
    /// Explicit password presence, if known.
    pub password_present: Option<bool>,
}
/// Original or effective evidence. Unrecognized imported kinds remain inspectable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fact {
    /// Recorded occurrence instant; imported unparseable values remain unchanged.
    pub time: String,
    /// Classified observation.
    pub kind: RecordedKind,
    /// Presence indicators only.
    pub presence: Presence,
}
/// One editable value supplied by an operator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Value {
    /// UTC instant with an explicit offset.
    pub time: String,
    /// Engagement or reporting event only.
    pub kind: Kind,
    /// Optional presence indicators; applicable only to submissions.
    pub presence: Presence,
}
/// Mutation intent; revisions and retry identities are mandatory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    /// Caller-generated identity retained unchanged after an uncertain response.
    pub request_id: String,
    /// Target identity; null only for an addition.
    pub event_id: Option<i64>,
    /// Reviewed event revision; zero for an uncorrected observation or addition.
    pub revision: i64,
    /// The operation to perform.
    pub action: Action,
    /// Required for add/edit and forbidden for exclude/restore.
    pub value: Option<Value>,
}
/// Restore returns to the original values and includes the event again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Add an operator entry.
    Add,
    /// Replace occurrence/type/presence on a specific event.
    Edit,
    /// Exclude one event from effective results.
    Exclude,
    /// Restore its original values and inclusion.
    Restore,
}
/// Original evidence and current correction state for a stable identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// Persistent event identity.
    pub id: i64,
    /// More specific system source, or `operator`.
    pub source: String,
    /// Latest committed correction revision.
    pub revision: i64,
    /// Immutable evidence.
    pub original: Fact,
    /// Current effective values, also retained while excluded.
    pub current: Fact,
    /// Whether this event is omitted from effective results.
    pub excluded: bool,
}
impl Record {
    /// Corrections do not change the event's origin.
    pub fn edited(&self) -> bool {
        let same_time = match (
            datetime::parse_stored(&self.original.time),
            datetime::parse_stored(&self.current.time),
        ) {
            (Ok(a), Ok(b)) => a == b,
            _ => self.original.time == self.current.time,
        };
        !same_time
            || self.original.kind != self.current.kind
            || self.original.presence != self.current.presence
    }
    /// SMTP, lifecycle and unrecognized imported records are read-only.
    pub fn editable(&self) -> bool {
        matches!(self.original.kind, RecordedKind::Known(kind) if editable(kind))
    }
}
/// One immutable audit revision, with server action time separate from occurrence time.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Audit {
    /// Revision committed by this action.
    pub revision: i64,
    /// Authenticated operator, including the real actor during impersonation.
    pub actor: i64,
    /// Server UTC action seconds.
    pub performed_at: i64,
    /// Mutation intent.
    pub action: Action,
    /// Previous state; absent for an addition.
    pub before: Option<Record>,
    /// Committed state.
    pub after: Record,
}
/// Bounded audit inspection alongside the current record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inspection {
    /// Current state, independent of the audit page.
    pub record: Record,
    /// Up to fifty most recent revisions in this page.
    pub audit: Vec<Audit>,
    /// Exclusive revision cursor for older audit records.
    pub next_before: Option<i64>,
}
/// Supported result observations; SMTP and lifecycle retain their own controls.
pub const fn editable(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::EmailOpened | Kind::LinkClicked | Kind::FormSubmitted | Kind::EmailReported
    )
}
/// Validate structure before storage and normalize only newly supplied instants.
pub fn validate(
    change: &Change,
    start: Option<datetime::Timestamp>,
    end: Option<datetime::Timestamp>,
    now: i64,
) -> Result<Option<Fact>, &'static str> {
    if !(16..=64).contains(&change.request_id.len())
        || !change
            .request_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || change.revision < 0
        || change.event_id.is_some_and(|id| id <= 0)
        || (change.action == Action::Add) != change.event_id.is_none()
        || (change.action == Action::Add && change.revision != 0)
        || matches!(change.action, Action::Add | Action::Edit) != change.value.is_some()
    {
        return Err("Invalid event change");
    }
    let Some(value) = &change.value else {
        return Ok(None);
    };
    if !editable(value.kind)
        || (value.kind != Kind::FormSubmitted && value.presence != Presence::default())
    {
        return Err("Presence indicators apply only to form submissions");
    }
    let time = datetime::parse(&value.time).map_err(|_| "Invalid occurrence time")?;
    if time == datetime::ZERO_TIMESTAMP
        || time.as_second() > now
        || start.is_some_and(|v| time < v)
        || end.is_some_and(|v| time > v)
    {
        return Err("Occurrence time is outside the campaign timeline");
    }
    Ok(Some(Fact {
        time: datetime::utc(time),
        kind: RecordedKind::Known(value.kind),
        presence: value.presence.clone(),
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Independent validation examples")]
mod tests {
    use super::*;

    #[test]
    fn occurrence_boundaries_preserve_fractional_instants_and_normalize_offsets() {
        let mut change = Change {
            request_id: "boundary-example-1".into(),
            event_id: None,
            revision: 0,
            action: Action::Add,
            value: Some(Value {
                time: "2026-06-01T02:00:00.500+02:00".into(),
                kind: Kind::LinkClicked,
                presence: Presence::default(),
            }),
        };
        let start = datetime::parse("2026-06-01T00:00:00.500Z").unwrap();
        let end = datetime::parse("2026-06-01T00:00:01.500Z").unwrap();
        let now = end.as_second() + 1;
        assert_eq!(
            validate(&change, Some(start), Some(end), now)
                .unwrap()
                .unwrap()
                .time,
            "2026-06-01T00:00:00.5Z"
        );
        for invalid in [
            "2026-06-01T00:00:00.499Z",
            "2026-06-01T00:00:01.501Z",
            "2026-06-01T00:00:00",
            "not-a-time",
        ] {
            change.value.as_mut().unwrap().time = invalid.into();
            assert!(validate(&change, Some(start), Some(end), now).is_err());
        }
        change.value.as_mut().unwrap().time = "2026-06-01T00:00:03Z".into();
        assert!(validate(&change, None, None, now).is_err());
        change.value.as_mut().unwrap().time = "2026-06-01T00:00:01.500Z".into();
        assert!(validate(&change, Some(start), Some(end), now).is_ok());
        change.value.as_mut().unwrap().presence.password_present = Some(false);
        assert!(validate(&change, None, None, now).is_err());
        change.value.as_mut().unwrap().kind = Kind::FormSubmitted;
        assert!(validate(&change, None, None, now).is_ok());
        change.value.as_mut().unwrap().kind = Kind::EmailSent;
        assert!(validate(&change, None, None, now).is_err());
    }
}
