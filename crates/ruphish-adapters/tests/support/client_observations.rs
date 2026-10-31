//! Coarse categories persist across providers, edits, retries and exports.
use super::*;
use ruphish_domain::client::{Browser, Device, Info, OperatingSystem};

pub(super) async fn exercise(
    db: &Database,
    actor: auth::Identity,
    mut input: campaign::Create,
    now: i64,
) {
    input.idempotency = None;
    let campaigns = db.campaigns();
    let campaign = campaigns.create(actor, input, now).await.unwrap();
    let recipient = &campaign.results[0];
    let public = db.interactions();
    let desktop = Info {
        browser: Browser::Firefox,
        os: OperatingSystem::Linux,
        device: Device::Desktop,
    };
    let phone = Info {
        browser: Browser::Safari,
        os: OperatingSystem::Ios,
        device: Device::Mobile,
    };
    for action in [
        interactions::Action::Open,
        interactions::Action::Report,
        interactions::Action::View { client: desktop },
        interactions::Action::View {
            client: Info::default(),
        },
        interactions::Action::Submit {
            client: phone,
            event_id: "d".repeat(32),
            password_present: true,
        },
        interactions::Action::Submit {
            client: desktop,
            event_id: "d".repeat(32),
            password_present: false,
        },
    ] {
        public
            .interact(
                "https://training.example.test",
                &recipient.rid,
                action,
                now + 1,
            )
            .await
            .unwrap();
    }
    let saved = campaigns.get(actor, campaign.id).await.unwrap();
    let submitted = saved
        .events
        .iter()
        .find(|e| e.message == "Submitted Data")
        .unwrap();
    assert_eq!(
        submitted.client,
        Some(phone),
        "retry cannot replace original request categories"
    );
    assert_eq!(
        saved
            .events
            .iter()
            .filter(|e| e.message == "Submitted Data")
            .count(),
        1
    );
    for e in &saved.events {
        if !matches!(e.message.as_str(), "Clicked Link" | "Submitted Data") {
            assert_eq!(e.client, None);
        }
    }
    let history = campaigns
        .recipient_history(actor, campaign.id, recipient.id, None, None)
        .await
        .unwrap();
    assert_eq!(
        history
            .events
            .iter()
            .find(|e| e.id == submitted.id)
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .client,
        Some(phone)
    );
    let changed = db
        .campaigns()
        .change_event(
            actor,
            campaign.id,
            recipient.id,
            ruphish_domain::event_edit::Change {
                request_id: "client-event-edit-1".into(),
                event_id: Some(submitted.id),
                revision: 0,
                action: ruphish_domain::event_edit::Action::Edit,
                value: Some(ruphish_domain::event_edit::Value {
                    time: submitted.time.clone(),
                    kind: ruphish_domain::events::Kind::LinkClicked,
                    presence: Default::default(),
                }),
            },
            now + 2,
        )
        .await
        .unwrap();
    assert_eq!(
        changed.event.unwrap().client,
        Some(phone),
        "editing an event keeps immutable request information"
    );
    let analysis = db
        .reporting()
        .analysis(
            actor,
            campaign.id,
            reporting::Filter {
                limit: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        analysis
            .events
            .iter()
            .find(|e| e.id == submitted.id)
            .unwrap()
            .client,
        Some(phone)
    );
    let csv = db
        .reporting()
        .export(
            actor,
            campaign.id,
            reporting::Filter {
                limit: 50,
                ..Default::default()
            },
            "csv",
            "events",
        )
        .await
        .unwrap();
    let mut reader = csv::Reader::from_reader(csv.bytes.as_slice());
    let columns = reader.headers().unwrap().clone();
    let index = |name| columns.iter().position(|c| c == name).unwrap();
    let rows: Vec<_> = reader.records().map(Result::unwrap).collect();
    let row = rows
        .iter()
        .find(|r| r[index("Event ID")] == submitted.id.to_string())
        .unwrap();
    assert_eq!(&row[index("Browser")], "safari");
    assert_eq!(&row[index("Operating system")], "ios");
    assert_eq!(&row[index("Device")], "mobile");
    assert!(rows.iter().any(|r| &r[index("Browser")] == "unknown"));
    assert!(
        rows.iter()
            .filter(|r| &r[index("Event")] == "Email Opened")
            .all(|r| r[index("Browser")].is_empty())
    );
    let excel = db
        .reporting()
        .export(
            actor,
            campaign.id,
            reporting::Filter {
                limit: 50,
                ..Default::default()
            },
            "xlsx",
            "events",
        )
        .await
        .unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(excel.bytes)).unwrap();
    let mut xml = String::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        if file.name().unwrap().ends_with(".xml") {
            std::io::Read::read_to_string(&mut file, &mut xml).unwrap();
        }
    }
    for value in [
        "Browser",
        "Operating system",
        "Device",
        "safari",
        "ios",
        "mobile",
    ] {
        assert!(xml.contains(value), "missing {value}");
    }
    campaigns
        .complete(actor, campaign.id, now + 3)
        .await
        .unwrap();
    assert!(
        public
            .interact(
                "https://training.example.test",
                &recipient.rid,
                interactions::Action::View { client: desktop },
                now + 4
            )
            .await
            .is_err()
    );
    assert_eq!(
        campaigns
            .get(actor, campaign.id)
            .await
            .unwrap()
            .events
            .len(),
        saved.events.len()
    );
    campaigns.delete(actor, campaign.id).await.unwrap();
}
