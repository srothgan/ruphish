//! Direct edits to individual events; automatic observations continue independently.
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
/// Current event values. Unrecognized imported kinds remain read-only.
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
    /// Caller-generated identity used to deduplicate creation retries.
    pub request_id: String,
    /// Target identity; null only for an addition.
    pub event_id: Option<i64>,
    /// Reviewed row version; zero for a new event.
    pub revision: i64,
    /// The operation to perform.
    pub action: Action,
    /// Required for add/edit and forbidden for delete.
    pub value: Option<Value>,
}
/// The supported direct mutations of one event row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Create a manually recorded event.
    Add,
    /// Update the current values.
    Edit,
    /// Permanently delete the event.
    Delete,
}
/// Current event row, with no retained original or audit revisions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// Immutable coarse request categories; absent for historical or non-client observations.
    pub client: Option<crate::client::Info>,
    /// Persistent event identity.
    pub id: i64,
    /// Origin stays unchanged when an event is edited.
    pub source: String,
    /// Row version for rejecting concurrent edits; not an edited indicator.
    pub revision: i64,
    /// True after an operator has actually changed this event's values.
    pub edited: bool,
    /// Current event values.
    pub current: Fact,
}
impl Record {
    /// SMTP, lifecycle and unrecognized imported records are read-only.
    pub fn editable(&self) -> bool {
        matches!(self.current.kind, RecordedKind::Known(kind) if editable(kind))
    }
}
impl Fact {
    /// Equal instants with different UTC offsets are unchanged values.
    pub fn same_values(&self, other: &Self) -> bool {
        let same_time = match (
            datetime::parse_stored(&self.time),
            datetime::parse_stored(&other.time),
        ) {
            (Ok(a), Ok(b)) => a == b,
            _ => self.time == other.time,
        };
        same_time && self.kind == other.kind && self.presence == other.presence
    }
}
/// Confirmed mutation result. Deleted rows have no current record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    /// Affected identity, including for deletion.
    pub id: i64,
    /// Current row after create/update; null after deletion.
    pub event: Option<Record>,
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
