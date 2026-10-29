//! Filter boundaries exercised through each real reporting provider.
use super::*;

pub async fn contract(db: &Database, actor: auth::Identity, campaign: i64, recipient: i64) {
    let base = || reporting::Filter {
        limit: 1,
        ..Default::default()
    };
    for (label, filter) in [
        ("zero rows", reporting::Filter { limit: 0, ..base() }),
        (
            "excess rows",
            reporting::Filter {
                limit: 201,
                ..base()
            },
        ),
        (
            "equal window",
            reporting::Filter {
                from: Some(10),
                until: Some(10),
                ..base()
            },
        ),
        (
            "reversed window",
            reporting::Filter {
                from: Some(11),
                until: Some(10),
                ..base()
            },
        ),
        (
            "excess offset",
            reporting::Filter {
                offset: 1_000_001,
                ..base()
            },
        ),
        (
            "excess search bytes",
            reporting::Filter {
                search: "\u{e9}".repeat(129),
                ..base()
            },
        ),
        (
            "excess selection",
            reporting::Filter {
                recipients: vec![recipient; 201],
                ..base()
            },
        ),
        (
            "zero identity",
            reporting::Filter {
                recipients: vec![0],
                ..base()
            },
        ),
        (
            "negative identity",
            reporting::Filter {
                recipients: vec![-1],
                ..base()
            },
        ),
    ] {
        assert!(
            matches!(db.reporting().analysis(actor, campaign, filter).await,
            Err(Error::Invalid(message)) if message == "analysis_invalid_filter"),
            "{label}"
        );
    }
    let ordinary = db
        .reporting()
        .analysis(actor, campaign, base())
        .await
        .unwrap();
    assert_eq!(ordinary.audience, 1);
    assert_eq!(ordinary.recipient_total, 1);
    assert_eq!(ordinary.recipients[0].id, recipient);
    let boundary = reporting::Filter {
        limit: 200,
        offset: 1_000_000,
        search: "\u{e9}".repeat(128),
        recipients: vec![recipient; 200],
        from: Some(10),
        until: Some(11),
        ..base()
    };
    let empty = db
        .reporting()
        .analysis(actor, campaign, boundary)
        .await
        .unwrap();
    assert_eq!(
        empty.audience, 1,
        "filters do not change the campaign audience"
    );
    assert_eq!(empty.recipient_total, 0);
    assert!(empty.recipients.is_empty());
    assert!(empty.events.is_empty());
    assert!(
        matches!(
            db.reporting()
                .analysis(
                    actor,
                    campaign,
                    reporting::Filter {
                        recipients: vec![i64::MAX],
                        ..base()
                    }
                )
                .await,
            Err(Error::NotFound("Recipient"))
        ),
        "accepted bounds still require campaign-owned identities"
    );
}
