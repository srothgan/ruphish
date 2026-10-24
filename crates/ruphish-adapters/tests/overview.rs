//! Real migrated SQLite, production read operations and independent expectations.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated functional fixture setup and assertions"
)]
mod support;
use ruphish_application::{auth::Identity, overview::*, resources::Error};
use sqlx::SqlitePool;
use std::sync::Arc;
use support::DatabaseFixture;
const NOW: i64 = 2_000_000_000;
const OWNER: Identity = Identity { id: 1, epoch: 0 };
async fn setup() -> (tempfile::TempDir, DatabaseFixture, Arc<dyn Repository>) {
    let dir = tempfile::tempdir().unwrap();
    let db = DatabaseFixture::open(&dir.path().join("overview.db"), "")
        .await
        .unwrap();
    let pool = db.inspection_pool();
    sqlx::raw_sql("INSERT INTO users(id,username,api_key,role_id,password_change_required,account_locked) VALUES(1,'owner','one-key',1,0,0),(2,'other','two-key',2,0,0); INSERT INTO ruphish_auth_state VALUES(1,0),(2,0);
        INSERT INTO campaigns(id,user_id,name,status,launch_date) VALUES(1,1,'Owner','In progress','2026-01-01T00:00:00Z'),(2,2,'Other owner','In progress','2026-01-01T00:00:00Z');")
        .execute(&pool).await.unwrap();
    let repo = db.overview();
    (dir, db, repo)
}
async fn recipient(pool: &SqlitePool, campaign: i64, email: &str) {
    sqlx::query("INSERT INTO results(campaign_id,user_id,email,status) VALUES(?,?,?,'Email Sent')")
        .bind(campaign)
        .bind(campaign)
        .bind(email)
        .execute(pool)
        .await
        .unwrap();
}
async fn event(
    pool: &SqlitePool,
    campaign: i64,
    email: &str,
    kind: &str,
    time: i64,
    details: &str,
) {
    sqlx::query("INSERT INTO events(campaign_id,email,kind,time,metadata,recipient_id) VALUES(?,?,?,datetime(?,'unixepoch'),?,(SELECT min(id) FROM results WHERE campaign_id=? AND email=? HAVING count(*)=1))")
        .bind(campaign).bind(email).bind(match kind { "Email Sent"=>"email_sent", "Email Opened"=>"email_opened", "Clicked Link"=>"link_clicked", "Submitted Data"=>"form_submitted", "Email Reported"=>"email_reported", other=>other }).bind(time).bind(details).bind(campaign).bind(email).execute(pool).await.unwrap();
}
#[tokio::test]
async fn acceptance_cohorts_deduplicate_isolate_owners_and_reject_untrusted_presence_values() {
    let (_dir, db, repo) = setup().await;
    let pool = db.inspection_pool();
    for (email, at) in [
        ("old", NOW - 31 * 86400),
        ("month", NOW - 30 * 86400),
        ("week", NOW - 7 * 86400),
        ("today", NOW - 86400),
        ("now", NOW),
        ("future", NOW + 1),
    ] {
        recipient(&pool, 1, email).await;
        event(&pool, 1, email, "Email Sent", at, "").await;
    }
    // A repeated send in the new period must not move an older recipient into it.
    event(&pool, 1, "old", "Email Sent", NOW, "").await;
    for email in ["old", "month", "week", "week"] {
        event(&pool, 1, email, "Email Opened", NOW, "").await;
    }
    event(&pool, 1, "today", "Clicked Link", NOW, "").await;
    event(&pool, 1, "week", "Email Reported", NOW, "").await;
    for email in ["today", "today", "now"] {
        event(
            &pool,
            1,
            email,
            "Submitted Data",
            NOW,
            r#"{"password_present":true,"password":"PRIVATE-CANARY"}"#,
        )
        .await;
    }
    for (email, details) in [
        ("old", "malformed PRIVATE-CANARY"),
        ("month", r#"{"password_present":1}"#),
        ("week", r#"{"password_present":"true"}"#),
    ] {
        event(&pool, 1, email, "Submitted Data", NOW, details).await;
    }
    event(&pool, 1, "week", "Clicked Link", NOW + 1, "").await;
    recipient(&pool, 1, "unsent").await;
    event(&pool, 1, "unsent", "Clicked Link", NOW, "").await;
    event(&pool, 1, "unknown-recipient", "Email Sent", NOW, "").await;
    for _ in 0..2 {
        recipient(&pool, 1, "ambiguous").await;
    }
    event(&pool, 1, "ambiguous", "Email Sent", NOW, "").await;
    event(&pool, 1, "ambiguous", "Clicked Link", NOW, "").await;
    recipient(&pool, 2, "today").await;
    event(&pool, 2, "today", "Email Sent", NOW, "").await;
    event(&pool, 2, "today", "Email Opened", NOW, "").await;
    for (window, sent, opened) in [
        (Window::All, 5, 5),
        (Window::Last30Days, 4, 4),
        (Window::Last7Days, 3, 3),
    ] {
        assert_eq!(
            repo.posture(OWNER, window, NOW).await.unwrap().counts,
            Posture {
                delivered: sent,
                opened,
                clicked: sent,
                reported: 1,
                credentials_entered: 2
            }
        );
    }
    assert_eq!(
        repo.posture(Identity { id: 2, epoch: 0 }, Window::All, NOW)
            .await
            .unwrap()
            .counts,
        Posture {
            delivered: 1,
            opened: 1,
            ..Default::default()
        }
    );
    assert_eq!(
        repo.posture(OWNER, Window::Last7Days, NOW + 40 * 86400)
            .await
            .unwrap()
            .counts,
        Posture::default()
    );
    // Reading never sanitizes by rewriting history or emits another event.
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM event_history WHERE details LIKE '%PRIVATE-CANARY%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 4);
    db.close().await;
}
#[tokio::test]
async fn snapshot_tracks_queued_launches_expiry_current_delivery_issues_and_inventory() {
    let (_dir, db, repo) = setup().await;
    let pool = db.inspection_pool();
    let settings = db.settings().get(OWNER).await.unwrap();
    let mut policy = settings.policy;
    policy.verification_enabled = true;
    let settings = db
        .settings()
        .save(OWNER, settings.revision, policy, NOW)
        .await
        .unwrap();

    for (id, status, at) in [
        (3, "Queued", NOW + 10),
        (4, "Scheduled", NOW + 50),
        (5, "Queued", NOW - 1),
        (6, "Completed", NOW + 20),
        (7, "In progress", NOW - 100),
    ] {
        sqlx::query("INSERT INTO campaigns(id,user_id,name,status,launch_date) VALUES(?,1,?,?,datetime(?,'unixepoch'))").bind(id).bind(format!("Campaign {id}")).bind(status).bind(at).execute(&pool).await.unwrap();
    }
    for (id, enabled, until) in [
        (1, 1, 0),
        (2, 1, NOW - 1),
        (3, 1, NOW + 1),
        (4, 0, NOW + 1),
        (5, 1, NOW),
    ] {
        sqlx::query("INSERT INTO ruphish_domains(id,owner_id,name,origin,hostname,enabled,challenge,challenge_expires,verified_until,due,checked_at) VALUES(?,1,'Domain',?, 'test',?,'secret',0,?,?,?)")
            .bind(id).bind(format!("https://{id}.test")).bind(enabled).bind(until).bind(NOW+1000).bind(if until>0 {until-86400}else{0}).execute(&pool).await.unwrap();
    }
    sqlx::raw_sql("INSERT INTO templates(user_id,name) VALUES(1,'Owned'),(2,'Other'); INSERT INTO pages(user_id,name) VALUES(1,'Owned'); INSERT INTO groups(user_id,name) VALUES(1,'One'),(1,'Two'); INSERT INTO smtp(user_id,name) VALUES(1,'One');
        INSERT INTO ruphish_mailboxes(owner_id,name,enabled,host,port,username,password,tls,folder) VALUES(1,'Reports',0,'test',993,'secret','SERVICE-CANARY',1,'INBOX');
        INSERT INTO ruphish_delivery(result_id,campaign_id,state,due) VALUES(1,1,'failed',0),(2,1,'blocked',0),(3,1,'unknown',0),(4,1,'cancelled',0),(5,6,'failed',0),(6,2,'failed',0);")
        .execute(&pool).await.unwrap();
    let s = repo.snapshot(OWNER, NOW).await.unwrap();
    assert_eq!(
        s.campaigns,
        CampaignCounts {
            total: 6,
            active: 3,
            scheduled: 2,
            completed: 1
        }
    );
    assert_eq!(
        s.recent.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![7, 6, 5, 4, 3]
    );
    assert_eq!(s.attention.next_launch.as_ref().map(|n| n.id), Some(3));
    assert_eq!(s.attention.delivery_problems, 3);
    assert_eq!(s.attention.unverified_enabled_domains, 3);
    assert_eq!(
        s.inventory,
        Inventory {
            templates: 1,
            pages: 1,
            groups: 2,
            profiles: 1,
            domains: 5,
            domains_verified: 2,
            reporting_mailboxes: 1
        }
    );
    let later = repo.snapshot(OWNER, NOW + 10).await.unwrap();
    assert_eq!(later.campaigns.scheduled, 1);
    assert_eq!(later.campaigns.active, 4);
    assert_eq!(later.attention.next_launch.as_ref().map(|n| n.id), Some(4));
    assert_eq!(later.inventory.domains_verified, 0);
    assert_eq!(later.attention.unverified_enabled_domains, 4);
    let mut policy = settings.policy;
    policy.verification_enabled = false;
    db.settings()
        .save(OWNER, settings.revision, policy, NOW + 11)
        .await
        .unwrap();
    let untracked = repo.snapshot(OWNER, NOW + 11).await.unwrap();
    assert_eq!(untracked.attention.unverified_enabled_domains, 0);
    assert_eq!(untracked.inventory.domains_verified, 0);
    assert_eq!(untracked.inventory.domains, 5);
    db.close().await;
}
#[tokio::test]
async fn empty_data_revoked_identities_and_missing_permissions_are_not_fake_zero_successes() {
    let (_dir, db, repo) = setup().await;
    let pool = db.inspection_pool();
    sqlx::raw_sql("DELETE FROM campaigns;")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        repo.snapshot(OWNER, NOW).await.unwrap(),
        Snapshot {
            as_of: NOW,
            ..Default::default()
        }
    );
    assert_eq!(
        repo.posture(OWNER, Window::All, NOW).await.unwrap().counts,
        Posture::default()
    );
    sqlx::query("ALTER TABLE ruphish_domains RENAME TO unavailable_domains")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        repo.snapshot(OWNER, NOW).await,
        Err(Error::Storage(
            ruphish_application::storage::Error::Unavailable
        ))
    ));
    sqlx::query("ALTER TABLE unavailable_domains RENAME TO ruphish_domains")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(repo.snapshot(OWNER, NOW).await.unwrap().campaigns.total, 0);
    for caller in [Identity { id: 999, epoch: 0 }, Identity { id: 1, epoch: 1 }] {
        assert!(repo.snapshot(caller, NOW).await.is_err());
        assert!(repo.posture(caller, Window::All, NOW).await.is_err());
    }
    sqlx::query("UPDATE users SET account_locked=1 WHERE id=1")
        .execute(&pool)
        .await
        .unwrap();
    assert!(repo.snapshot(OWNER, NOW).await.is_err());
    sqlx::query("UPDATE users SET account_locked=0,role_id=999 WHERE id=1")
        .execute(&pool)
        .await
        .unwrap();
    assert!(repo.posture(OWNER, Window::All, NOW).await.is_err());
    db.close().await;
}

#[tokio::test]
async fn the_same_address_in_two_campaigns_counts_separately_without_fabricated_events() {
    let (_dir, db, repo) = setup().await;
    let pool = db.inspection_pool();
    sqlx::query(
        "INSERT INTO campaigns(id,user_id,name,status) VALUES(3,1,'Second campaign','In progress')",
    )
    .execute(&pool)
    .await
    .unwrap();
    for campaign in [1, 3] {
        recipient(&pool, campaign, "same@example.test").await;
        event(&pool, campaign, "same@example.test", "Email Sent", NOW, "").await;
        event(
            &pool,
            campaign,
            "same@example.test",
            "Clicked Link",
            NOW,
            "",
        )
        .await;
    }
    // A link can be fetched while the SMTP acceptance result is being persisted.
    // The observation still belongs to this sent recipient; summary progression does not create an open event.
    event(&pool, 1, "same@example.test", "Email Reported", NOW - 1, "").await;
    assert_eq!(
        repo.posture(OWNER, Window::Last7Days, NOW)
            .await
            .unwrap()
            .counts,
        Posture {
            delivered: 2,
            opened: 2,
            clicked: 2,
            reported: 1,
            ..Default::default()
        }
    );
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM event_history")
        .fetch_one(&pool)
        .await
        .unwrap();
    repo.snapshot(OWNER, NOW).await.unwrap();
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM event_history")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, after);
    db.close().await;
}

#[tokio::test]
async fn comparisons_use_disjoint_periods_and_do_not_rewrite_previous_outcomes_with_late_events() {
    for (window, days) in [
        (Window::Last7Days, 7),
        (Window::Last30Days, 30),
        (Window::LastYear, 365),
    ] {
        let (_dir, db, repo) = setup().await;
        let pool = db.inspection_pool();
        let boundary = NOW - days * 86400;
        let start = NOW - 2 * days * 86400;
        for (email, at) in [
            ("too-old", start - 1),
            ("first", start),
            ("last", boundary - 1),
            ("current", boundary),
            ("latest", NOW),
            ("future", NOW + 1),
        ] {
            recipient(&pool, 1, email).await;
            event(&pool, 1, email, "Email Sent", at, "").await;
            event(&pool, 1, email, "Email Opened", at, "").await;
        }
        // Repeated sends cannot move a previous recipient into the current cohort.
        event(&pool, 1, "first", "Email Sent", NOW, "").await;
        // The prior period is observed only through its own end.
        event(&pool, 1, "first", "Clicked Link", boundary - 1, "").await;
        event(&pool, 1, "first", "Clicked Link", boundary - 1, "").await;
        event(&pool, 1, "last", "Clicked Link", boundary, "").await;
        event(&pool, 1, "last", "Email Reported", NOW, "").await;
        event(
            &pool,
            1,
            "first",
            "Submitted Data",
            boundary - 1,
            r#"{"password_present":true}"#,
        )
        .await;
        event(
            &pool,
            1,
            "last",
            "Submitted Data",
            boundary,
            r#"{"password_present":true}"#,
        )
        .await;
        recipient(&pool, 2, "first").await;
        event(&pool, 2, "first", "Email Sent", start, "").await;
        event(&pool, 2, "first", "Email Reported", start, "").await;
        let metrics = repo.posture(OWNER, window, NOW).await.unwrap();
        assert_eq!(
            metrics.counts,
            Posture {
                delivered: 2,
                opened: 2,
                ..Default::default()
            }
        );
        assert_eq!(
            metrics.previous,
            Some(PreviousPeriod {
                since: start,
                until: boundary,
                counts: Posture {
                    delivered: 2,
                    opened: 2,
                    clicked: 1,
                    reported: 0,
                    credentials_entered: 1
                }
            })
        );
        let all = repo.posture(OWNER, Window::All, NOW).await.unwrap();
        assert!(all.previous.is_none());
        assert_eq!(all.counts.delivered, 5);
        assert_eq!(all.counts.reported, 1);
        let empty = repo
            .posture(OWNER, window, NOW + 800 * 86400)
            .await
            .unwrap();
        assert_eq!(empty.counts, Posture::default());
        assert_eq!(empty.previous.unwrap().counts, Posture::default());
        db.close().await;
    }
}
