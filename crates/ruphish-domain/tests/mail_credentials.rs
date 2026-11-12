//! Saved credentials remain usable only within their connection boundary.
#![allow(
    clippy::unwrap_used,
    reason = "Synthetic policy fixtures and assertions"
)]
use ruphish_domain::{
    integrations::MailboxEdit,
    mail_credentials::Connection,
    resources::{Content, Input, Profile, Resource},
    validation::Code,
};

fn saved(host: &str) -> Resource {
    Resource {
        attribution: Default::default(),
        revision: 1,
        id: 1,
        name: "Shared relay".into(),
        modified_date: String::new(),
        content: Content::Profile(Profile {
            host: host.into(),
            username: "service".into(),
            password: "saved-canary".into(),
            from_address: "sender@example.test".into(),
            ..Default::default()
        }),
    }
}

#[test]
fn smtp_credentials_reject_rebinding_but_allow_explicit_replacement_and_clear() {
    let original = saved("mail.example.test:465");
    for field in ["host", "port", "username", "certificate"] {
        let mut draft: Input = original.clone().map_password(|_| None::<String>).into();
        if let Content::Profile(profile) = &mut draft.content {
            match field {
                "host" => profile.host = "other.example.test:465".into(),
                "port" => profile.host = "mail.example.test:587".into(),
                "username" => profile.username = "other".into(),
                _ => profile.ignore_cert_errors = true,
            }
        }
        let error = draft
            .clone()
            .resolve_content(Some(&original))
            .err()
            .unwrap();
        assert_eq!(error.issues().len(), 1);
        assert_eq!(error.issues()[0].field, "password");
        assert_eq!(error.issues()[0].code, Code::CredentialReentryRequired);
        assert!(!error.to_string().contains("saved-canary"));
        for replacement in ["replacement-canary", ""] {
            let mut explicit = draft.clone();
            if let Content::Profile(profile) = &mut explicit.content {
                profile.password = Some(replacement.into());
            }
            assert!(
                matches!(explicit.resolve_content(Some(&original)).unwrap(), Content::Profile(p) if p.password == replacement)
            );
        }
    }
}

#[test]
fn equivalent_smtp_endpoints_and_non_security_edits_preserve_credentials() {
    for (before, after) in [
        ("MAIL.example.test", "mail.example.test:25"),
        ("[2001:db8::1]:465", "[2001:0db8:0:0:0:0:0:1]:465"),
    ] {
        let original = saved(before);
        let mut draft: Input = original.clone().map_password(|_| None::<String>).into();
        draft.name = "Renamed".into();
        if let Content::Profile(profile) = &mut draft.content {
            profile.host = after.into();
            profile.from_address = "other-sender@example.test".into();
        }
        assert!(
            matches!(draft.resolve_content(Some(&original)).unwrap(), Content::Profile(p) if p.password == "saved-canary")
        );
    }
    let mut original = saved("mail.example.test:465");
    if let Content::Profile(p) = &mut original.content {
        p.ignore_cert_errors = true;
    }
    let mut draft: Input = original.clone().map_password(|_| None::<String>).into();
    if let Content::Profile(p) = &mut draft.content {
        p.ignore_cert_errors = false;
    }
    assert!(
        matches!(draft.resolve_content(Some(&original)).unwrap(), Content::Profile(p) if p.password == "saved-canary")
    );
    let original = original.map_password(|_| String::new());
    let mut draft: Input = original.clone().map_password(|_| None::<String>).into();
    if let Content::Profile(p) = &mut draft.content {
        p.host = "another.example.test:587".into();
    }
    assert!(
        matches!(draft.resolve_content(Some(&original)).unwrap(), Content::Profile(p) if p.password.is_empty())
    );
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
fn previous() -> Connection<'static> {
    Connection {
        host: "mail.example.test",
        port: 993,
        username: "service",
        implicit_tls: true,
        ignore_cert_errors: false,
    }
}

#[test]
fn mailbox_credentials_are_bound_to_endpoint_identity_and_tls() {
    for field in ["host", "port", "username", "tls", "certificate"] {
        let mut edit = mailbox();
        match field {
            "host" => edit.host = "other.example.test".into(),
            "port" => edit.port = 143,
            "username" => edit.username = "other".into(),
            "tls" => edit.tls = false,
            _ => edit.ignore_cert_errors = true,
        }
        let error = edit
            .resolve_password(previous(), "mailbox-canary")
            .unwrap_err();
        assert_eq!(error.issues()[0].code, Code::CredentialReentryRequired);
        edit.password = Some("replacement-canary".into());
        assert_eq!(
            edit.resolve_password(previous(), "mailbox-canary").unwrap(),
            "replacement-canary"
        );
    }
    let mut edit = mailbox();
    edit.host = "MAIL.EXAMPLE.TEST".into();
    edit.name = "Renamed reports".into();
    edit.folder = "Other folder".into();
    edit.frequency = 120;
    edit.enabled = false;
    assert_eq!(
        edit.resolve_password(previous(), "mailbox-canary").unwrap(),
        "mailbox-canary"
    );
}
