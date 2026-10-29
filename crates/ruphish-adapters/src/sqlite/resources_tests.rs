//! Physical work guarantees over the real schema and repository, not just result rows.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[tokio::test]
async fn group_summary_work_does_not_scale_with_unrelated_memberships()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("summary-work.db");
    let database = crate::sqlite::StartupDatabase::open(&path, "").await?;
    database.close().await;
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(path))
        .await?;
    sqlx::raw_sql(
        "INSERT INTO users(id,username,api_key,role_id,password_change_required,account_locked) VALUES(1,'owner','key1',1,FALSE,FALSE),(2,'other','key2',2,FALSE,FALSE);
         INSERT INTO ruphish_auth_state VALUES(1,0),(2,0);
         INSERT INTO groups(id,user_id,name,modified_date) VALUES(10,1,'Selected','2026-01-01T00:00:00Z'),(11,1,'Empty','2026-01-01T00:00:00Z'),(20,2,'Unrelated','2026-01-01T00:00:00Z');
         INSERT INTO targets(id,email) VALUES(1,'one@example.test'),(2,'two@example.test');
         INSERT INTO group_targets(group_id,target_id) VALUES(10,1),(10,2);"
    ).execute(&pool).await?;
    let repository = Resources::new(pool.clone());
    let owner = Identity { id: 1, epoch: 0 };
    let selections = [None, Some(10), Some(11), Some(20), Some(999)];
    // Warm the actual statement cache before measuring execution, excluding setup.
    for id in selections {
        repository.summaries(owner, Kind::Group, id).await?;
    }
    let instructions = Arc::new(AtomicUsize::new(0));
    let mut connection = pool.acquire().await?;
    let version: String = sqlx::query_scalar("SELECT sqlite_version()")
        .fetch_one(&mut *connection)
        .await?;
    let counter = instructions.clone();
    connection
        .lock_handle()
        .await?
        .set_progress_handler(1, move || {
            counter.fetch_add(1, Ordering::Relaxed);
            true
        });
    drop(connection);
    let mut work = Vec::new();
    let mut plans = Vec::new();
    for unrelated in [0, 10_000, 200_000] {
        if unrelated > 0 {
            sqlx::raw_sql("DELETE FROM group_targets WHERE group_id=20;
                WITH RECURSIVE ids(id) AS (SELECT 1 UNION ALL SELECT id+1 FROM ids WHERE id<200000)
                INSERT OR IGNORE INTO targets(id,email) SELECT id+2,'other-' || id || '@example.test' FROM ids;")
                .execute(&pool).await?;
            sqlx::query("INSERT INTO group_targets(group_id,target_id) SELECT 20,id FROM targets WHERE id>2 AND id<=?+2")
                .bind(unrelated).execute(&pool).await?;
        }
        let mut measured = Vec::new();
        for id in selections {
            instructions.store(0, Ordering::Relaxed);
            let rows = repository.summaries(owner, Kind::Group, id).await?;
            measured.push(instructions.load(Ordering::Relaxed));
            let actual: Vec<_> = rows
                .iter()
                .map(|row| (row.id, row.num_targets, row.active_campaigns))
                .collect();
            let expected = match id {
                None => vec![(10, 2, 0), (11, 0, 0)],
                Some(10) => vec![(10, 2, 0)],
                Some(11) => vec![(11, 0, 0)],
                _ => vec![],
            };
            assert_eq!(actual, expected);
            let plan = sqlx::query(sqlx::AssertSqlSafe(format!(
                "EXPLAIN QUERY PLAN {}",
                summary_query(Kind::Group)
            )))
            .bind(owner.id)
            .bind(id)
            .bind(id)
            .fetch_all(&pool)
            .await?
            .iter()
            .map(|row| row.try_get::<String, _>("detail"))
            .collect::<Result<Vec<_>, _>>()?;
            println!("SQLite {version}, unrelated={unrelated}, selected={id:?}: {plan:?}");
            plans.push(plan);
        }
        println!(
            "SQLite {version}, unrelated={unrelated}, VM instructions (list/used/empty/foreign/missing): {measured:?}"
        );
        work.push(measured);
    }
    pool.acquire()
        .await?
        .lock_handle()
        .await?
        .remove_progress_handler();
    pool.close().await;
    for plan in plans {
        assert!(
            plan.iter().any(|step| step
                .contains("SEARCH g USING COVERING INDEX ruphish_group_targets_group")
                && step.contains("group_id=?")),
            "Memberships must be sought by selected group ID: {plan:?}"
        );
        assert!(
            !plan.iter().any(|step| step.starts_with("SCAN g ")),
            "Unrelated memberships must not be scanned: {plan:?}"
        );
    }
    for measured in &work[1..] {
        for (baseline, actual) in work[0].iter().zip(measured) {
            // Permit fixed planner/transaction overhead, never work proportional to
            // the 10,000/200,000 unrelated rows. No wall-clock threshold is involved.
            assert!(
                *actual <= baseline + 200,
                "Unrelated memberships increased VM work: {work:?}"
            );
        }
    }
    Ok(())
}
