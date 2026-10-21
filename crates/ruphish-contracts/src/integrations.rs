//! Required integration response fields; credentials are never part of these types.
use serde::{Deserialize, Serialize};

/// Administrator-visible webhook configuration.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Webhook {
    /// Persistent identifier.
    pub id: i64,
    /// Operator-visible name.
    pub name: String,
    /// Destination URL.
    pub url: String,
    /// Whether event delivery is enabled.
    pub is_active: bool,
    /// Optimistic concurrency revision.
    pub revision: i64,
    /// Whether a signing secret is configured; never its value.
    pub has_secret: bool,
    /// Recent processing attempts.
    pub history: Vec<Run>,
}

/// Owner-visible mailbox configuration and worker status.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Mailbox {
    /// Persistent identifier.
    pub id: i64,
    /// Operator-visible name.
    pub name: String,
    /// Whether polling is enabled.
    pub enabled: bool,
    /// IMAP hostname.
    pub host: String,
    /// IMAP port.
    pub port: u16,
    /// Configured service account name.
    pub username: String,
    /// Whether a service password is configured; never its value.
    pub has_password: bool,
    /// Whether to use implicit TLS.
    pub tls: bool,
    /// Monitored mailbox folder.
    pub folder: String,
    /// Optional sender-domain restriction.
    pub restrict_domain: String,
    /// Whether matched messages are removed.
    pub delete_reported: bool,
    /// Polling interval in seconds.
    pub frequency: i64,
    /// Optimistic concurrency revision.
    pub revision: i64,
    /// Last check time in UTC Unix seconds.
    pub last_check: i64,
    /// Last successful check in UTC Unix seconds.
    pub last_success: i64,
    /// Next scheduled check in UTC Unix seconds.
    pub next_check: i64,
    /// Whether a worker currently owns a check.
    pub running: bool,
    /// Application-owned outcome code.
    pub code: String,
    /// Recent processing attempts.
    pub history: Vec<Run>,
}

/// A completed or in-progress integration attempt.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Run {
    /// Persistent identifier.
    pub id: i64,
    /// Associated webhook event, when applicable.
    #[serde(deserialize_with = "crate::nullable")]
    pub event_id: Option<i64>,
    /// Webhook delivery attempt number, when applicable.
    #[serde(deserialize_with = "crate::nullable")]
    pub attempt: Option<i64>,
    /// Attempt start in UTC Unix seconds.
    pub started: i64,
    /// Completion time; absent while in progress.
    #[serde(deserialize_with = "crate::nullable")]
    pub finished: Option<i64>,
    /// HTTP response status, when applicable.
    #[serde(deserialize_with = "crate::nullable")]
    pub status: Option<i64>,
    /// Application-owned outcome code.
    pub code: String,
    /// Number of correlated reports.
    pub matched: i64,
    /// Number of rejected reports.
    pub rejected: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_run_outcomes_are_not_confirmed_pending_work() {
        let complete = json!({"id":1,"event_id":null,"attempt":null,"started":123,"finished":null,"status":null,"code":"queued","matched":0,"rejected":0});
        assert!(serde_json::from_value::<Run>(complete.clone()).is_ok());
        for field in ["event_id", "attempt", "finished", "status"] {
            let mut value = complete.clone();
            value.as_object_mut().map(|v| v.remove(field));
            assert!(serde_json::from_value::<Run>(value).is_err());
        }
    }

    #[test]
    fn incomplete_integration_responses_cannot_become_disabled_or_empty() {
        let complete = json!({"id":3,"name":"Events","url":"https://example.test/hook","is_active":false,"revision":2,"has_secret":false,"history":[]});
        assert!(serde_json::from_value::<Webhook>(complete.clone()).is_ok());
        for field in [
            "id",
            "name",
            "url",
            "is_active",
            "revision",
            "has_secret",
            "history",
        ] {
            let mut value = complete.clone();
            value.as_object_mut().map(|v| v.remove(field));
            assert!(serde_json::from_value::<Webhook>(value).is_err());
        }
        assert!(serde_json::from_value::<Vec<Webhook>>(json!({})).is_err());
        assert!(serde_json::from_value::<Mailbox>(json!({"id":3,"enabled":false})).is_err());
        assert!(serde_json::from_value::<Run>(json!({"id":1,"started":12,"code":"ok"})).is_err());
    }
}
