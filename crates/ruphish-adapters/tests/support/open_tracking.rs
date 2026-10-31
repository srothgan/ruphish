//! Campaign policy survives preparation, persistence, delivery and report reads.
use super::*;
use ruphish_application::delivery::Sender;

pub(super) async fn exercise(
    db: &Database,
    actor: auth::Identity,
    mut input: campaign::Create,
    now: i64,
) {
    let campaigns = db.campaigns();
    let filter = || reporting::Filter {
        limit: 50,
        ..Default::default()
    };
    assert!(input.policy.track_opens);
    input.idempotency = None;
    for enabled in [false, true] {
        input.policy.track_opens = enabled;
        let prepared = campaigns.prepare(actor, &input, now).await.unwrap();
        assert_eq!(prepared.policy.track_opens, enabled);
        let campaign = campaigns.create(actor, input.clone(), now).await.unwrap();
        assert_eq!(campaign.policy.as_ref().unwrap().track_opens, enabled);
        let claim = campaigns.claim(now).await.unwrap().unwrap();
        assert_eq!(claim.campaign_id, campaign.id);
        assert_eq!(claim.message.track_opens, enabled);
        let mail = ruphish_adapters::smtp::Smtp::new(Policy::default(), String::new())
            .compose(&claim.message)
            .unwrap();
        let parsed = mail_parser::MessageParser::default()
            .parse(&mail.bytes)
            .unwrap();
        let html = parsed.body_html(0).unwrap();
        assert_eq!(html.contains("/track?"), enabled);
        let public = db.interactions();
        let rid = &campaign.results[0].rid;
        for _ in 0..2 {
            assert!(matches!(
                public
                    .interact(
                        "https://training.example.test",
                        rid,
                        interactions::Action::Open,
                        now
                    )
                    .await
                    .unwrap(),
                interactions::Output::Pixel
            ));
        }
        let recorded = campaigns.get(actor, campaign.id).await.unwrap();
        if !enabled {
            assert_eq!(recorded.results[0].status, campaign.results[0].status);
            assert_eq!(recorded.events.len(), campaign.events.len());
        }
        assert_eq!(
            recorded
                .events
                .iter()
                .filter(|e| e.message == "Email Opened")
                .count(),
            if enabled { 2 } else { 0 }
        );
        public
            .interact(
                "https://training.example.test",
                rid,
                interactions::Action::View {
                    client: Default::default(),
                },
                now,
            )
            .await
            .unwrap();
        for action in [
            interactions::Action::Submit {
                client: Default::default(),
                event_id: "a".repeat(32),
                password_present: false,
            },
            interactions::Action::Report,
        ] {
            public
                .interact("https://training.example.test", rid, action, now)
                .await
                .unwrap();
        }
        let analysis = db
            .reporting()
            .analysis(actor, campaign.id, filter())
            .await
            .unwrap();
        assert_eq!(analysis.track_opens, enabled);
        assert_eq!(analysis.totals[1], if enabled { 2 } else { 0 });
        assert_eq!(
            analysis.totals[2], 1,
            "clicks remain independent of open tracking"
        );
        assert_eq!(
            &analysis.totals[3..],
            &[1, 1],
            "submissions and reports remain independent of open tracking"
        );
        let csv = db
            .reporting()
            .export(actor, campaign.id, filter(), "csv", "recipients")
            .await
            .unwrap();
        let mut reader = csv::Reader::from_reader(csv.bytes.as_slice());
        let row = reader.records().next().unwrap().unwrap();
        assert_eq!(&row[5], if enabled { "2" } else { "Not tracked" });
        let excel = db
            .reporting()
            .export(actor, campaign.id, filter(), "xlsx", "recipients")
            .await
            .unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(excel.bytes)).unwrap();
        for (sheet, untracked_cells) in [("sheet1.xml", 2), ("sheet2.xml", 1)] {
            let mut xml = String::new();
            std::io::Read::read_to_string(
                &mut archive.by_name(&format!("xl/worksheets/{sheet}")).unwrap(),
                &mut xml,
            )
            .unwrap();
            assert_eq!(
                xml.matches("Not tracked").count(),
                if enabled { 0 } else { untracked_cells }
            );
        }
        campaigns.complete(actor, campaign.id, now).await.unwrap();
        campaigns.delete(actor, campaign.id).await.unwrap();
    }
}
