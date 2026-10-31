//! Domain administration wire types, independent of infrastructure and secret state.
use serde::{Deserialize, Serialize};

/// Decode a closed resource/event object, never Serde's positional struct array form.
pub fn decode_request<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
) -> Result<T, serde_json::Error> {
    if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
        return Err(<serde_json::Error as serde::de::Error>::custom(
            "Expected a JSON object",
        ));
    }
    serde_json::from_slice(bytes)
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
/// Owner-visible domain resource and derived readiness.
pub struct Domain {
    /// Effective policy, separate from historical observations.
    pub verification: Verification,
    /// Persistent resource identifier.
    pub id: i64,
    /// Operator-visible display name.
    pub name: String,
    /// Canonical public HTTP(S) origin.
    pub origin: String,
    /// Canonical ASCII DNS name or IP address without IPv6 brackets.
    pub hostname: String,
    /// Explicit operational enablement.
    pub enabled: bool,
    /// Optimistic concurrency revision.
    pub revision: i64,
    /// DNS TXT record name to publish; empty for IP origins.
    pub txt_name: String,
    /// Exact DNS TXT record value to publish; empty for IP origins.
    pub txt_value: String,
    /// Challenge expiry as UTC Unix seconds.
    pub challenge_expires: i64,
    /// Last completed check as UTC Unix seconds.
    pub checked_at: i64,
    /// Expiry of the last complete successful verification.
    pub verified_until: i64,
    /// Latest DNS control observation.
    pub control: bool,
    /// Latest permitted resolution observation.
    pub dns: bool,
    /// Latest instance routing observation.
    pub routing: bool,
    /// Latest HTTPS observation.
    pub tls: bool,
    /// Closed operational result code.
    pub code: String,
    /// Server-derived verification validity, independent of enabled state.
    pub status: String,
    /// Server-derived availability for campaign selection.
    pub eligible: bool,
    /// Whether a persisted verification lease is active.
    pub checking: bool,
    /// Current queued or running attempt, independent of domain validity.
    #[serde(deserialize_with = "crate::nullable")]
    pub active_check: Option<VerificationRun>,
    /// Latest ten completed attempts, newest first; populated on detail responses.
    pub history: Vec<VerificationRun>,
    /// Next automatic or requested check, in UTC Unix seconds.
    pub next_check_at: i64,
    /// Number of referencing campaigns.
    pub campaigns: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
/// Create or update metadata with optimistic concurrency.
pub struct Edit {
    /// Operator-visible display name.
    pub name: String,
    /// Canonical public HTTP(S) origin.
    pub origin: String,
    /// Explicit operational enablement.
    pub enabled: bool,
    /// Optimistic concurrency revision.
    pub revision: Option<i64>,
}

/// Participant values have no representation in this closed schema.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    /// Opaque idempotency identifier; contains no field values.
    pub event_id: String,
    /// Whether a designated password control was nonempty.
    pub password_present: bool,
}

/// Durable verification attempt; only closed result codes are exposed.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct VerificationRun {
    /// Persistent attempt identity.
    pub id: i64,
    /// Queued, running, passed, failed or timed_out.
    pub state: String,
    /// Earliest scheduled execution, in UTC Unix seconds.
    pub scheduled_at: i64,
    /// Actual worker start; absent for queued or imported historical checks.
    #[serde(deserialize_with = "crate::nullable")]
    pub started_at: Option<i64>,
    /// Actual completion; absent while active.
    #[serde(deserialize_with = "crate::nullable")]
    pub finished_at: Option<i64>,
    /// Safe application-owned result code.
    pub code: String,
}

#[cfg(test)]
mod read_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn verification_times_require_explicit_null_for_queued_work() {
        let complete = json!({"id":1,"state":"queued","scheduled_at":123,"started_at":null,"finished_at":null,"code":"queued"});
        assert!(serde_json::from_value::<VerificationRun>(complete.clone()).is_ok());
        for field in ["started_at", "finished_at"] {
            let mut value = complete.clone();
            value.as_object_mut().map(|v| v.remove(field));
            assert!(serde_json::from_value::<VerificationRun>(value).is_err());
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
/// Effective verification under the current company policy.
pub struct Verification {
    /// Whether the installation currently requires verification.
    pub required: bool,
    /// Effective verification state under current policy.
    pub status: String,
    /// UTC Unix deadline, absent for unrestricted evidence validity.
    pub expires_at: Option<i64>,
    /// Next verification check, absent while verification is disabled.
    pub next_check_at: Option<i64>,
}
