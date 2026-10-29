//! Decisions remain independent of the provider and transport implementation.
#![allow(clippy::unwrap_used, reason = "Independent policy assertions")]

use ruphish_domain::{
    account::*,
    campaign::{DeliveryState, resolve_delivery},
    integrations::*,
    validation::{Code, Report},
};

#[test]
fn manual_recovery_never_retries_an_uncertain_delivery_without_explicit_consent() {
    assert_eq!(
        resolve_delivery(DeliveryState::Unknown, "confirm_accepted").unwrap(),
        DeliveryState::Accepted
    );
    assert_eq!(
        resolve_delivery(DeliveryState::Unknown, "resend_unknown").unwrap(),
        DeliveryState::Ready
    );
    for state in [DeliveryState::Failed, DeliveryState::Blocked] {
        assert_eq!(
            resolve_delivery(state, "retry").unwrap(),
            DeliveryState::Ready
        );
    }
    for (state, action) in [
        (DeliveryState::Unknown, "retry"),
        (DeliveryState::Accepted, "retry"),
        (DeliveryState::Submitting, "retry"),
        (DeliveryState::Ready, "confirm_accepted"),
        (DeliveryState::Failed, "confirm_accepted"),
        (DeliveryState::Unknown, "invented"),
    ] {
        assert!(resolve_delivery(state, action).is_err());
    }
}

#[test]
fn webhook_policy_combines_transport_evidence_budget_and_delay() {
    for status in [
        None,
        Some(408),
        Some(429),
        Some(500),
        Some(502),
        Some(503),
        Some(504),
    ] {
        let decision = webhook_decision("webhook_failed", status, 7, 31);
        assert_eq!(decision.state, "retry");
        assert_eq!(decision.delay, 3601);
        assert_eq!(
            webhook_decision("webhook_failed", status, 8, 0).state,
            "failed"
        );
    }
    for status in [Some(400), Some(401), Some(403), Some(404), Some(501)] {
        assert_eq!(
            webhook_decision("webhook_failed", status, 1, 0).state,
            "failed"
        );
    }
    for code in [
        "webhook_invalid_url",
        "webhook_signing_failed",
        "configuration_changed",
    ] {
        assert_eq!(webhook_decision(code, None, 1, 0).state, "failed");
    }
    assert_eq!(
        webhook_decision("accepted", Some(204), 8, 0).state,
        "accepted"
    );
    assert_eq!(webhook_decision("webhook_failed", None, 1, 29).delay, 89);
    assert_eq!(mailbox_delay(true, 300, 8), 300);
    assert_eq!(mailbox_delay(false, 300, 0), 60);
    assert_eq!(mailbox_delay(false, 300, 1), 120);
    assert_eq!(mailbox_delay(false, 300, u32::MAX), 3600);
}

fn mailbox() -> MailboxEdit {
    MailboxEdit {
        name: "Reports".into(),
        enabled: true,
        host: "mail.example.test".into(),
        port: 993,
        username: "service".into(),
        password: None,
        tls: true,
        ignore_cert_errors: false,
        folder: "INBOX".into(),
        restrict_domain: String::new(),
        delete_reported: false,
        frequency: 60,
        revision: Some(1),
    }
}

fn diagnostics(report: Report, expected: &[(&str, Code)], message: &str) {
    let report = report.finish().unwrap_err();
    assert_eq!(report.issues().len(), expected.len());
    for (field, code) in expected {
        assert!(
            report
                .issues()
                .iter()
                .any(|issue| issue.field == *field && issue.code == *code),
            "Missing diagnostic {field}: {code:?} in {report:?}"
        );
    }
    assert_eq!(report.message(), message);
}

#[test]
fn configuration_bounds_and_checkpoint_identity_are_preserved() {
    let mut edit = mailbox();
    assert!(edit.validation().finish().is_ok());
    assert!(!edit.resets_checkpoint("mail.example.test", 993, "service", "INBOX"));
    for (host, port, user, folder) in [
        ("other.example.test", 993, "service", "INBOX"),
        ("mail.example.test", 143, "service", "INBOX"),
        ("mail.example.test", 993, "other", "INBOX"),
        ("mail.example.test", 993, "service", "Reports"),
    ] {
        assert!(edit.resets_checkpoint(host, port, user, folder));
    }
    edit.name = "Renamed".into();
    edit.password = Some("rotated".into());
    edit.tls = false;
    edit.enabled = false;
    edit.delete_reported = true;
    edit.frequency = 120;
    assert!(!edit.resets_checkpoint("mail.example.test", 993, "service", "INBOX"));
    for frequency in [30, 31_540_000] {
        edit.frequency = frequency;
        assert!(edit.validation().finish().is_ok());
    }
    for frequency in [0, 29, 31_540_001] {
        edit.frequency = frequency;
        diagnostics(
            edit.validation(),
            &[("frequency", Code::Invalid)],
            "mailbox_invalid_configuration",
        );
    }
    edit = mailbox();
    edit.host = "mail.example.test/path".into();
    diagnostics(
        edit.validation(),
        &[("host", Code::Invalid)],
        "mailbox_invalid_configuration",
    );
    let mut hook = WebhookEdit {
        name: "Events".into(),
        url: "https://receiver.example.test/hook".into(),
        secret: None,
        active: true,
        revision: None,
    };
    assert!(hook.validation().finish().is_ok());
    hook.secret = Some(String::new());
    diagnostics(
        hook.validation(),
        &[("secret", Code::Invalid)],
        "webhook_secret_required",
    );
    hook.secret = None;
    hook.url = "https://user:pass@receiver.example.test".into();
    diagnostics(
        hook.validation(),
        &[("url", Code::Invalid)],
        "webhook_invalid_configuration",
    );
    hook.url = "not a URL".into();
    diagnostics(
        hook.validation(),
        &[("url", Code::InvalidUrl)],
        "webhook_invalid_url",
    );
    // Independent failures remain visible together, instead of collapsing to
    // the old wrapper's first coarse enum variant.
    hook.name.clear();
    hook.secret = Some(String::new());
    diagnostics(
        hook.validation(),
        &[
            ("name", Code::Invalid),
            ("url", Code::InvalidUrl),
            ("secret", Code::Invalid),
        ],
        "webhook_invalid_url",
    );
    edit.frequency = 0;
    diagnostics(
        edit.validation(),
        &[("host", Code::Invalid), ("frequency", Code::Invalid)],
        "mailbox_invalid_configuration",
    );
}

#[test]
fn edit_privileges_follow_permissions_instead_of_role_names() {
    let mut actor = Account {
        preferences: Default::default(),
        identity: Identity { id: 1, epoch: 0 },
        username: "editor".into(),
        hash: String::new(),
        api_key: String::new(),
        role: Role {
            id: 2,
            slug: "custom".into(),
            name: "Custom".into(),
            description: String::new(),
            permissions: vec!["modify_objects".into()],
        },
        password_change_required: false,
        account_locked: false,
        last_login: String::new(),
    };
    let existing = actor.clone();
    let mut edit = Edit {
        username: "renamed".into(),
        password: String::new(),
        role: "custom".into(),
        password_change_required: false,
        account_locked: false,
    };
    assert!(actor.authorize_edit(&existing, &edit).is_ok());
    edit.role = "admin".into();
    assert!(matches!(
        actor.authorize_edit(&existing, &edit),
        Err(AccountError::Escalation)
    ));
    actor.role.permissions.push("modify_system".into());
    assert!(actor.authorize_edit(&existing, &edit).is_ok());
    actor.role.permissions.clear();
    edit.role = "custom".into();
    edit.account_locked = true;
    assert!(matches!(
        actor.authorize_edit(&existing, &edit),
        Err(AccountError::Escalation)
    ));
    edit.account_locked = false;
    edit.password_change_required = true;
    assert!(matches!(
        actor.authorize_edit(&existing, &edit),
        Err(AccountError::Escalation)
    ));
}
