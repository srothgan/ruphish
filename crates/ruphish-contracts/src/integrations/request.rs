//! Request contracts shared by HTTP handlers and browser clients.
use serde::{Deserialize, Serialize};

/// Webhook configuration write.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebhookWrite {
    /// Operator-supplied display name.
    pub name: String,
    /// Configured destination URL.
    pub url: String,
    /// Replacement signing secret; omission preserves the saved secret.
    pub secret: Option<String>,
    #[serde(default)]
    /// Whether event delivery is enabled.
    pub is_active: bool,
    /// Expected saved revision; absent only for creation.
    pub revision: Option<i64>,
}

/// Reporting-mailbox configuration write.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MailboxWrite {
    /// Operator-supplied display name.
    pub name: String,
    /// Whether processing is enabled.
    pub enabled: bool,
    /// Service hostname.
    pub host: String,
    /// Service port.
    pub port: u16,
    /// Service login name.
    pub username: String,
    /// Replacement credential; omission preserves it only within its saved connection boundary.
    pub password: Option<String>,
    /// Use implicit TLS; false requests STARTTLS.
    pub tls: bool,
    #[serde(default)]
    /// Explicit certificate-verification exception.
    pub ignore_cert_errors: bool,
    /// Monitored mailbox folder.
    pub folder: String,
    #[serde(default)]
    /// Optional sender-domain filter.
    pub restrict_domain: String,
    #[serde(default)]
    /// Remove successfully correlated messages.
    pub delete_reported: bool,
    /// Polling interval in seconds.
    pub frequency: u32,
    /// Expected saved revision; absent only for creation.
    pub revision: Option<i64>,
}

impl From<&super::Webhook> for WebhookWrite {
    fn from(value: &super::Webhook) -> Self {
        Self {
            name: value.name.clone(),
            url: value.url.clone(),
            secret: None,
            is_active: value.is_active,
            revision: Some(value.revision),
        }
    }
}
impl TryFrom<&super::Mailbox> for MailboxWrite {
    type Error = std::num::TryFromIntError;
    fn try_from(v: &super::Mailbox) -> Result<Self, Self::Error> {
        Ok(Self {
            name: v.name.clone(),
            enabled: v.enabled,
            host: v.host.clone(),
            port: v.port,
            username: v.username.clone(),
            password: None,
            tls: v.tls,
            ignore_cert_errors: v.ignore_cert_errors,
            folder: v.folder.clone(),
            restrict_domain: v.restrict_domain.clone(),
            delete_reported: v.delete_reported,
            frequency: v.frequency.try_into()?,
            revision: Some(v.revision),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn integration_writes_cannot_echo_read_only_fields_or_smuggle_identity() {
        let hook = json!({"name":"Receiver","url":"https://example.test/events","revision":7});
        let mailbox = json!({"name":"Reports","enabled":true,"host":"example.test","port":993,"username":"reporter","tls":true,"folder":"INBOX","frequency":60,"revision":7});
        assert!(serde_json::from_value::<WebhookWrite>(hook.clone()).is_ok());
        assert!(serde_json::from_value::<MailboxWrite>(mailbox.clone()).is_ok());
        for key in [
            "id",
            "owner_id",
            "created_at",
            "created_by",
            "edited_by",
            "edited_at",
            "history",
            "has_password",
            "has_secret",
        ] {
            let mut forged = hook.clone();
            forged[key] = json!(999);
            assert!(serde_json::from_value::<WebhookWrite>(forged).is_err());
            let mut forged = mailbox.clone();
            forged[key] = json!(999);
            assert!(serde_json::from_value::<MailboxWrite>(forged).is_err());
        }
    }
}
