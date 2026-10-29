//! Actual provider reads and downloaded artifacts across page boundaries.
use super::*;
use ruphish_domain::{
    pagination::{Cursor as SortedCursor, DeliverySort, DirectorySort, SortedPage},
    reporting::{EventSort, RecipientSort, SortDirection},
};

const SETUP: &str = "UPDATE users SET password_change_required=false;
INSERT INTO campaigns(id,user_id,name,status,launch_date) VALUES(900,1,'Same','Completed','2026-01-01T00:00:00Z'),(901,1,'Same','Completed','2026-01-01T02:00:00+02:00'),(902,1,'','Completed',NULL),(903,1,'\u{e000}','Completed','2026-01-01T00:00:00.000000001Z'),(904,1,'\u{1f600}','Completed','2026-01-01T00:00:00.000000002Z');
INSERT INTO results(id,campaign_id,user_id,r_id,email,first_name,last_name,status,reported) VALUES(9001,900,1,'sort-1','z@example.test','Same','','Email Sent',false),(9002,900,1,'sort-2','a@example.test','Same','','Email Sent',false),(9003,900,1,'sort-3','c@example.test','','','Email Sent',false),(9004,900,1,'sort-4','b@example.test','\u{e000}','','Email Sent',false),(9005,900,1,'sort-5','d@example.test','\u{1f600}','','Email Sent',true);
INSERT INTO ruphish_delivery(result_id,campaign_id,state,due) VALUES(9001,900,'accepted',10),(9002,900,'failed',2),(9003,900,'accepted',2),(9004,900,'cancelled',5),(9005,900,'failed',1);
INSERT INTO events(id,campaign_id,recipient_id,email,time,kind,metadata,source) VALUES(9101,900,9002,'a@example.test','2026-01-01T00:00:00Z','email_opened','{}','public'),(9102,900,9002,'a@example.test','2026-01-01T02:00:00.000000001+02:00','email_opened','{}','public'),(9103,900,9004,'b@example.test','2026-01-01T00:00:00.000000002Z','email_opened','{}','public'),(9104,900,9005,'d@example.test','2026-01-01T00:00:03Z','form_submitted','{\"password_present\":true}','public'),(9105,900,9003,'c@example.test','2026-01-01T00:00:04Z','form_submitted','{\"password_present\":false}','public');";

async fn bootstrap_sorting(db: &Database) {
    db.bootstrap(
        Some("Sorting fixture password 42!".into()),
        Some("sorting-key".into()),
    )
    .await
    .unwrap();
}
async fn exercise(db: &Database) {
    let actor = db.accounts().by_name("admin").await.unwrap().identity;
    let store = db.campaigns();
    for (sort, ascending, descending) in [
        (
            DirectorySort::Name,
            vec![902, 900, 901, 903, 904],
            vec![904, 903, 900, 901, 902],
        ),
        (
            DirectorySort::Status,
            vec![900, 901, 902, 903, 904],
            vec![900, 901, 902, 903, 904],
        ),
        (
            DirectorySort::Total,
            vec![901, 902, 903, 904, 900],
            vec![900, 901, 902, 903, 904],
        ),
        (
            DirectorySort::Launch,
            vec![902, 900, 901, 903, 904],
            vec![904, 903, 900, 901, 902],
        ),
    ] {
        for (order, expected) in [
            (SortDirection::Asc, ascending),
            (SortDirection::Desc, descending),
        ] {
            let mut selection = SortedPage {
                sort,
                order,
                scope: "directory-fixture".into(),
                after: None,
            };
            let mut actual = Vec::new();
            loop {
                let page = store
                    .directory(
                        actor,
                        campaign::directory::Filter::Executions,
                        campaign::directory::Cursor {
                            sorted: Some(selection.clone()),
                            ..Default::default()
                        },
                        2,
                    )
                    .await
                    .unwrap();
                actual.extend(
                    page.items
                        .into_iter()
                        .map(|row| row.id.parse::<i64>().unwrap()),
                );
                let Some(next) = page.next_after else {
                    break;
                };
                let cursor: SortedCursor<DirectorySort> = serde_json::from_str(&next).unwrap();
                assert!(cursor.valid(&sort, order, "directory-fixture"));
                selection.after = Some(cursor.position);
                assert!(actual.len() <= 5, "cursor must advance");
            }
            assert_eq!(actual, expected);
        }
    }
    for (sort, ascending, descending) in [
        (
            DeliverySort::Email,
            vec![9002, 9004, 9003, 9005, 9001],
            vec![9001, 9005, 9003, 9004, 9002],
        ),
        (
            DeliverySort::State,
            vec![9001, 9003, 9004, 9002, 9005],
            vec![9002, 9005, 9004, 9001, 9003],
        ),
        (
            DeliverySort::Due,
            vec![9005, 9002, 9003, 9004, 9001],
            vec![9001, 9004, 9002, 9003, 9005],
        ),
        (
            DeliverySort::AttemptCount,
            vec![9001, 9002, 9003, 9004, 9005],
            vec![9001, 9002, 9003, 9004, 9005],
        ),
    ] {
        for (order, expected) in [
            (SortDirection::Asc, ascending),
            (SortDirection::Desc, descending),
        ] {
            let mut selection = SortedPage {
                sort,
                order,
                scope: "delivery-fixture".into(),
                after: None,
            };
            let mut actual = Vec::new();
            loop {
                let page = store
                    .deliveries_sorted(actor, 900, 0, 2, Some(selection.clone()))
                    .await
                    .unwrap();
                actual.extend(page.items.into_iter().map(|row| row.id));
                let Some(next) = page.next_cursor else {
                    break;
                };
                let cursor: SortedCursor<DeliverySort> = serde_json::from_str(&next).unwrap();
                assert!(cursor.valid(&sort, order, "delivery-fixture"));
                selection.after = Some(cursor.position);
                assert!(actual.len() <= 5);
            }
            assert_eq!(actual, expected);
        }
    }
    let reports = db.reporting();
    for (sort, ascending, descending) in [
        (
            RecipientSort::Email,
            vec![9002, 9004, 9003, 9005, 9001],
            vec![9001, 9005, 9003, 9004, 9002],
        ),
        (
            RecipientSort::Name,
            vec![9003, 9001, 9002, 9004, 9005],
            vec![9005, 9004, 9001, 9002, 9003],
        ),
        (
            RecipientSort::Delivery,
            vec![9001, 9003, 9004, 9002, 9005],
            vec![9002, 9005, 9004, 9001, 9003],
        ),
        (
            RecipientSort::Accepted,
            vec![9001, 9002, 9003, 9004, 9005],
            vec![9001, 9002, 9003, 9004, 9005],
        ),
        (
            RecipientSort::Opened,
            vec![9001, 9003, 9005, 9004, 9002],
            vec![9002, 9004, 9001, 9003, 9005],
        ),
        (
            RecipientSort::Clicked,
            vec![9001, 9002, 9003, 9004, 9005],
            vec![9001, 9002, 9003, 9004, 9005],
        ),
        (
            RecipientSort::Submitted,
            vec![9001, 9002, 9004, 9003, 9005],
            vec![9003, 9005, 9001, 9002, 9004],
        ),
        (
            RecipientSort::Reported,
            vec![9001, 9002, 9003, 9004, 9005],
            vec![9001, 9002, 9003, 9004, 9005],
        ),
    ] {
        for (order, expected) in [
            (SortDirection::Asc, ascending),
            (SortDirection::Desc, descending),
        ] {
            let mut actual = Vec::new();
            for offset in [0, 2, 4, 6] {
                let page = reports
                    .analysis(
                        actor,
                        900,
                        reporting::Filter {
                            sort,
                            order,
                            limit: 2,
                            offset,
                            ..Default::default()
                        },
                    )
                    .await
                    .unwrap();
                assert_eq!(page.recipient_total, 5);
                actual.extend(page.recipients.into_iter().map(|row| row.id));
            }
            assert_eq!(actual, expected);
            let download = reports
                .export(
                    actor,
                    900,
                    reporting::Filter {
                        sort,
                        order,
                        limit: 2,
                        offset: 2,
                        ..Default::default()
                    },
                    "csv",
                    "recipients",
                )
                .await
                .unwrap();
            let mut csv = csv::Reader::from_reader(download.bytes.as_slice());
            let column = csv
                .headers()
                .unwrap()
                .iter()
                .position(|v| v == "Recipient ID")
                .unwrap();
            let exported: Vec<i64> = csv
                .records()
                .map(|row| row.unwrap()[column].parse().unwrap())
                .collect();
            assert_eq!(exported, expected, "export must use complete sorted set");
        }
    }
    for (sort, order, expected) in [
        (
            EventSort::Time,
            SortDirection::Desc,
            vec![9105, 9104, 9103, 9102, 9101],
        ),
        (
            EventSort::Kind,
            SortDirection::Asc,
            vec![9101, 9102, 9103, 9104, 9105],
        ),
        (
            EventSort::Source,
            SortDirection::Desc,
            vec![9105, 9104, 9103, 9102, 9101],
        ),
        (
            EventSort::PasswordPresent,
            SortDirection::Asc,
            vec![9101, 9102, 9103, 9105, 9104],
        ),
    ] {
        let mut actual = Vec::new();
        for offset in [0, 2, 4] {
            let page = reports
                .analysis(
                    actor,
                    900,
                    reporting::Filter {
                        event_sort: sort,
                        event_order: order,
                        limit: 2,
                        offset,
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            actual.extend(page.events.into_iter().map(|row| row.id));
        }
        assert_eq!(actual, expected);
    }
    let filtered = reports
        .analysis(
            actor,
            900,
            reporting::Filter {
                sort: RecipientSort::Opened,
                order: SortDirection::Desc,
                event: vec![ruphish_domain::events::Kind::EmailOpened],
                limit: 1,
                offset: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(filtered.recipient_total, 2);
    assert_eq!(filtered.recipients[0].id, 9004);
    assert_eq!(
        reports
            .analysis(
                actor,
                900,
                reporting::Filter {
                    search: "no match".into(),
                    limit: 2,
                    ..Default::default()
                }
            )
            .await
            .unwrap()
            .recipient_total,
        0
    );

    // Union filters aggregate the complete selection before either table is paged.
    let union_filter = reporting::Filter {
        event: vec![
            ruphish_domain::events::Kind::EmailOpened,
            ruphish_domain::events::Kind::FormSubmitted,
        ],
        limit: 1,
        offset: 2,
        ..Default::default()
    };
    let union = reports
        .analysis(actor, 900, union_filter.clone())
        .await
        .unwrap();
    assert_eq!(union.event_total, 5);
    assert_eq!(union.recipient_total, 4);
    assert_eq!(union.totals, [0, 3, 0, 2, 0]);
    assert_eq!(union.unique, [0, 2, 0, 2, 0]);
    assert_eq!(
        union.first_observed,
        [None, Some(1767225600), None, Some(1767225603), None]
    );
    assert_eq!(
        union.last_observed,
        [None, Some(1767225600), None, Some(1767225604), None]
    );
    let export = reports
        .export(actor, 900, union_filter, "csv", "events")
        .await
        .unwrap();
    assert_eq!(
        csv::Reader::from_reader(export.bytes.as_slice())
            .records()
            .count(),
        5
    );
    let single = reports
        .analysis(
            actor,
            900,
            reporting::Filter {
                recipient: Some(9002),
                event: vec![
                    ruphish_domain::events::Kind::EmailOpened,
                    ruphish_domain::events::Kind::FormSubmitted,
                ],
                limit: 1,
                offset: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(single.totals, [0, 2, 0, 0, 0]);
    assert_eq!(single.first_observed[1], Some(1767225600));
    assert_eq!(single.last_observed[1], Some(1767225600));

    let window = reports
        .analysis(
            actor,
            900,
            reporting::Filter {
                event: vec![
                    ruphish_domain::events::Kind::EmailOpened,
                    ruphish_domain::events::Kind::FormSubmitted,
                ],
                from: Some(1767225600),
                until: Some(1767225601),
                limit: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        window.event_total, 3,
        "Offset timestamps must use their actual UTC second in filters as well as summaries"
    );
    assert_eq!(window.totals, [0, 3, 0, 0, 0]);

    // Mixed draft/execution identities remain deterministic for equal names and
    // absent audience/schedule values. A cursor must survive boundary deletion.
    let draft_id = "sorting-draft-00000001";
    store
        .save_draft(
            actor,
            draft_id,
            None,
            campaign::drafts::Configuration {
                name: "Same".into(),
                ..Default::default()
            },
            0,
            100,
        )
        .await
        .unwrap();
    let selection = SortedPage {
        sort: DirectorySort::Name,
        order: SortDirection::Asc,
        scope: "mixed-directory".into(),
        after: None,
    };
    let first = store
        .directory(
            actor,
            campaign::directory::Filter::All,
            campaign::directory::Cursor {
                sorted: Some(selection.clone()),
                ..Default::default()
            },
            2,
        )
        .await
        .unwrap();
    assert_eq!(
        first
            .items
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        ["902", draft_id]
    );
    let cursor: SortedCursor<DirectorySort> =
        serde_json::from_str(first.next_after.as_ref().unwrap()).unwrap();
    assert!(!cursor.valid(&DirectorySort::Total, SortDirection::Asc, "mixed-directory"));
    assert!(!cursor.valid(&DirectorySort::Name, SortDirection::Desc, "mixed-directory"));
    assert!(!cursor.valid(&DirectorySort::Name, SortDirection::Asc, "another-filter"));
    let total_first = store
        .directory(
            actor,
            campaign::directory::Filter::All,
            campaign::directory::Cursor {
                sorted: Some(SortedPage {
                    sort: DirectorySort::Total,
                    ..selection.clone()
                }),
                ..Default::default()
            },
            2,
        )
        .await
        .unwrap();
    assert_eq!(total_first.items[0].id, draft_id);
    assert!(total_first.items[0].total.is_none());
    store.delete_draft(actor, draft_id, 1).await.unwrap();
    let next = store
        .directory(
            actor,
            campaign::directory::Filter::All,
            campaign::directory::Cursor {
                sorted: Some(SortedPage {
                    after: Some(cursor.position),
                    ..selection
                }),
                ..Default::default()
            },
            2,
        )
        .await
        .unwrap();
    assert_eq!(
        next.items
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        ["900", "901"]
    );
}

// Native transport absence must be distinct from an ambiguous submitted attempt.
const MISSING_TRANSPORT: &str = "DELETE FROM ruphish_delivery WHERE result_id=9002; UPDATE ruphish_delivery SET state='unknown' WHERE result_id=9005;";
async fn historical_transport(db: &Database) {
    let actor = db.accounts().by_name("admin").await.unwrap().identity;
    for (order, expected) in [
        (SortDirection::Asc, vec![9002, 9001, 9003, 9004, 9005]),
        (SortDirection::Desc, vec![9005, 9004, 9001, 9003, 9002]),
    ] {
        let snapshot = db
            .reporting()
            .analysis(
                actor,
                900,
                reporting::Filter {
                    sort: RecipientSort::Delivery,
                    order,
                    limit: 50,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(
            snapshot
                .recipients
                .iter()
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            snapshot
                .recipients
                .iter()
                .find(|row| row.id == 9002)
                .unwrap()
                .delivery,
            None
        );
        assert_eq!(
            snapshot
                .recipients
                .iter()
                .find(|row| row.id == 9005)
                .unwrap()
                .delivery,
            Some(ruphish_domain::campaign::DeliveryState::Unknown)
        );
    }
}

// The selected event qualifies recipients; the time/source scope still bounds
// every returned occurrence, including actions before and after qualification.
const EVENT_SELECTION: &str = "
INSERT INTO campaigns(id,user_id,name,status) VALUES(950,1,'Event selection','Completed');
INSERT INTO results(id,campaign_id,user_id,r_id,email,status,reported) VALUES
(9501,950,1,'selection-1','a@example.test','Email Sent',false),
(9502,950,1,'selection-2','b@example.test','Email Sent',false),
(9503,950,1,'selection-3','c@example.test','Email Sent',false),
(9504,950,1,'selection-4','d@example.test','Email Sent',false);
INSERT INTO events(id,campaign_id,recipient_id,email,time,kind,metadata,source) VALUES
(95001,950,9501,'a@example.test','2026-01-01T00:00:00Z','email_opened','{}','public'),
(95002,950,9501,'a@example.test','2026-01-01T00:00:01Z','form_submitted','{}','public'),
(95003,950,9501,'a@example.test','2026-01-01T00:00:02Z','link_clicked','{}','public'),
(95004,950,9501,'a@example.test','2026-01-01T00:00:03Z','form_submitted','{}','public'),
(95005,950,9501,'a@example.test','2026-01-01T00:00:04Z','email_reported','{}','operator'),
(95006,950,9501,'a@example.test','2026-01-01T00:00:05Z','email_opened','{}','public'),
(95007,950,9502,'b@example.test','2025-12-31T23:59:59Z','form_submitted','{}','public'),
(95008,950,9502,'b@example.test','2026-01-01T00:00:02Z','link_clicked','{}','public'),
(95009,950,9502,'b@example.test','2026-01-01T00:00:05Z','form_submitted','{}','public'),
(95010,950,9503,'c@example.test','2026-01-01T00:00:00Z','email_reported','{}','public'),
(95011,950,9503,'c@example.test','2026-01-01T00:00:04Z','link_clicked','{}','public'),
(95012,950,NULL,'unattributed@example.test','2026-01-01T00:00:02Z','form_submitted','{}','legacy');";

async fn event_selection(db: &Database) {
    use ruphish_domain::events::Kind;
    let actor = db.accounts().by_name("admin").await.unwrap().identity;
    let reports = db.reporting();
    let filter = reporting::Filter {
        event: vec![Kind::FormSubmitted],
        from: Some(1767225600),
        until: Some(1767225605),
        event_order: SortDirection::Asc,
        limit: 50,
        ..Default::default()
    };
    let selected = reports.analysis(actor, 950, filter.clone()).await.unwrap();
    assert_eq!(selected.audience, 4);
    assert_eq!(selected.recipient_total, 1);
    assert_eq!(selected.recipients[0].id, 9501);
    assert_eq!(selected.event_total, 5);
    assert_eq!(
        selected.events.iter().map(|e| e.id).collect::<Vec<_>>(),
        [95001, 95002, 95003, 95004, 95005]
    );
    assert_eq!(selected.totals, [0, 1, 1, 2, 1]);
    assert_eq!(selected.unique, [0, 1, 1, 1, 1]);
    assert_eq!(selected.recipients[0].counts, [0, 1, 1, 2, 1]);
    assert_eq!(
        selected.first_observed,
        [
            None,
            Some(1767225600),
            Some(1767225602),
            Some(1767225601),
            Some(1767225604)
        ]
    );
    assert_eq!(selected.last_observed[3], Some(1767225603));
    assert_eq!(selected.buckets[0].counts, [0, 1, 1, 2, 1]);
    assert_eq!(selected.participant_buckets[0].counts, [0, 1, 1, 1, 1]);

    let union_filter = reporting::Filter {
        event: vec![Kind::FormSubmitted, Kind::EmailReported],
        limit: 1,
        offset: 1,
        ..filter.clone()
    };
    let union = reports
        .analysis(actor, 950, union_filter.clone())
        .await
        .unwrap();
    assert_eq!(union.recipient_total, 2);
    assert_eq!(union.recipients[0].id, 9503);
    assert_eq!(union.recipients[0].counts, [0, 0, 1, 0, 1]);
    assert_eq!(union.event_total, 7);
    assert_eq!(union.events.len(), 1);
    assert_eq!(union.totals, [0, 1, 2, 2, 2]);
    assert_eq!(union.unique, [0, 1, 2, 1, 2]);
    assert_eq!(union.buckets[0].counts, [0, 1, 2, 2, 2]);
    assert_eq!(union.participant_buckets[0].counts, [0, 1, 2, 1, 2]);
    for (scope, column, expected) in [
        (
            "events",
            "Event ID",
            vec![
                "95001", "95010", "95002", "95003", "95004", "95005", "95011",
            ],
        ),
        ("recipients", "Recipient ID", vec!["9501", "9503"]),
    ] {
        let download = reports
            .export(actor, 950, union_filter.clone(), "csv", scope)
            .await
            .unwrap();
        let mut reader = csv::Reader::from_reader(download.bytes.as_slice());
        let index = reader
            .headers()
            .unwrap()
            .iter()
            .position(|name| name == column)
            .unwrap();
        let ids = reader
            .records()
            .map(|row| row.unwrap()[index].to_owned())
            .collect::<Vec<_>>();
        assert_eq!(ids, expected);
    }

    let public = reports
        .analysis(
            actor,
            950,
            reporting::Filter {
                source: Some(reporting::Source::Public),
                ..filter.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!(public.totals, [0, 1, 1, 2, 0]);
    for restricted in [
        reporting::Filter {
            source: Some(reporting::Source::Operator),
            ..filter.clone()
        },
        reporting::Filter {
            recipients: vec![9502, 9503],
            ..filter.clone()
        },
        reporting::Filter {
            recipient: Some(9502),
            ..filter.clone()
        },
        reporting::Filter {
            search: "b@example.test".into(),
            ..filter.clone()
        },
        reporting::Filter {
            from: Some(1767225604),
            ..filter.clone()
        },
    ] {
        let empty = reports.analysis(actor, 950, restricted).await.unwrap();
        assert_eq!(empty.recipient_total, 0);
        assert_eq!(empty.event_total, 0);
        assert_eq!(empty.totals, [0; 5]);
        assert!(empty.participant_buckets.is_empty());
    }
    let all = reports
        .analysis(
            actor,
            950,
            reporting::Filter {
                event: vec![],
                ..filter
            },
        )
        .await
        .unwrap();
    assert_eq!(
        all.recipient_total, 4,
        "No event selection retains recipients without activity"
    );
    assert_eq!(
        all.event_total, 9,
        "Unattributed history remains available without event selection"
    );
}

#[tokio::test]
async fn sqlite_sorted_tables_cover_complete_sets_and_exports() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sorting.db");
    let config = Config::Sqlite { path: path.clone() };
    let db = database::open(config, Policy::default()).await.unwrap();
    bootstrap_sorting(&db).await;
    let mut inspect = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
    )
    .await
    .unwrap();
    sqlx::raw_sql(SETUP).execute(&mut inspect).await.unwrap();
    exercise(&db).await;
    sqlx::raw_sql(EVENT_SELECTION)
        .execute(&mut inspect)
        .await
        .unwrap();
    event_selection(&db).await;
    sqlx::raw_sql(MISSING_TRANSPORT)
        .execute(&mut inspect)
        .await
        .unwrap();
    historical_transport(&db).await;
    db.close().await;
}
#[tokio::test]
#[ignore = "Requires canonical MySQL fixture and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_sorted_tables_cover_complete_sets_and_exports() {
    let (mut inspect, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    bootstrap_sorting(&db).await;
    sqlx::raw_sql(SETUP).execute(&mut inspect).await.unwrap();
    exercise(&db).await;
    sqlx::raw_sql(EVENT_SELECTION)
        .execute(&mut inspect)
        .await
        .unwrap();
    event_selection(&db).await;
    sqlx::raw_sql(MISSING_TRANSPORT)
        .execute(&mut inspect)
        .await
        .unwrap();
    historical_transport(&db).await;
    db.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE `{}`",
        config.database
    )))
    .execute(&mut inspect)
    .await
    .unwrap();
}
#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_sorted_tables_cover_complete_sets_and_exports() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    bootstrap_sorting(&db).await;
    let mut inspect = postgres_inspect(&config).await;
    sqlx::raw_sql(SETUP).execute(&mut inspect).await.unwrap();
    exercise(&db).await;
    sqlx::raw_sql(EVENT_SELECTION)
        .execute(&mut inspect)
        .await
        .unwrap();
    event_selection(&db).await;
    sqlx::raw_sql(MISSING_TRANSPORT)
        .execute(&mut inspect)
        .await
        .unwrap();
    historical_transport(&db).await;
    db.close().await;
    inspect.close().await.unwrap();
    postgres_cleanup(&mut admin, &config).await;
}
