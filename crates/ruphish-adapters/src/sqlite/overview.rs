//! Owner-scoped dashboard aggregation over the existing database. Read-only, and
//! never reads entered participant values: credentials-entered counts only the
//! stored password-presence boolean.
use ruphish_application::{auth::Identity, overview::*, resources::Error};
use sqlx::{Row, SqlitePool};

/// Dashboard reads share the startup pool.
pub struct Overview {
    pool: SqlitePool,
}
impl Overview {
    /// Compose dashboard aggregation over the existing database.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}
fn storage(error: sqlx::Error) -> Error {
    crate::sqlite::resource_error(error)
}

#[async_trait::async_trait]
impl Repository for Overview {
    async fn snapshot(&self, caller: Identity, now: i64) -> Result<Snapshot, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        super::resources::authorize(&mut tx, caller, false).await?;
        let owner = caller.id;

        // The persisted future state is Queued; scheduled/active are derived at
        // the snapshot time, so a delayed worker cannot leave stale counts.
        let row = sqlx::query(
            "SELECT count(*) AS total, \
             COALESCE(sum(status='Completed'),0) AS completed, \
             COALESCE(sum(status!='Completed' AND COALESCE(unixepoch(launch_date),0)>?2),0) AS scheduled, \
             COALESCE(sum(status!='Completed' AND COALESCE(unixepoch(launch_date),0)<=?2),0) AS active \
             FROM campaigns WHERE user_id=?1",
        ).bind(owner).bind(now).fetch_one(&mut *tx).await.map_err(storage)?;
        let campaigns = CampaignCounts {
            total: row.try_get("total").map_err(storage)?,
            active: row.try_get("active").map_err(storage)?,
            scheduled: row.try_get("scheduled").map_err(storage)?,
            completed: row.try_get("completed").map_err(storage)?,
        };

        // Recent campaigns, newest first, with audience size.
        let rows = sqlx::query(
            "SELECT id,name,status,launch_date,\
             (SELECT count(*) FROM results r WHERE r.campaign_id=c.id) AS total \
             FROM campaigns c WHERE user_id=? ORDER BY id DESC LIMIT 5",
        )
        .bind(owner)
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let mut recent = Vec::with_capacity(rows.len());
        for r in rows {
            recent.push(RecentCampaign {
                id: r.try_get("id").map_err(storage)?,
                name: r.try_get("name").map_err(storage)?,
                status: r.try_get("status").map_err(storage)?,
                launch: r
                    .try_get::<Option<String>, _>("launch_date")
                    .map_err(storage)?
                    .unwrap_or_default(),
                recipients: r.try_get("total").map_err(storage)?,
                outcomes: aggregate(
                    &mut tx,
                    owner,
                    None,
                    now,
                    Some(r.try_get("id").map_err(storage)?),
                )
                .await?,
            });
        }

        // Resource inventory. Each owner-scoped count is an explicit literal query.
        let inventory = Inventory {
            reporting_mailboxes: sqlx::query_scalar(
                "SELECT count(*) FROM ruphish_mailboxes WHERE owner_id=?",
            )
            .bind(owner)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?,
            templates: sqlx::query_scalar("SELECT count(*) FROM templates WHERE user_id=?")
                .bind(owner)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?,
            pages: sqlx::query_scalar("SELECT count(*) FROM pages WHERE user_id=?")
                .bind(owner)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?,
            groups: sqlx::query_scalar("SELECT count(*) FROM groups WHERE user_id=?")
                .bind(owner)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?,
            profiles: sqlx::query_scalar("SELECT count(*) FROM smtp WHERE user_id=?")
                .bind(owner)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?,
            domains: sqlx::query_scalar("SELECT count(*) FROM ruphish_domains WHERE owner_id=?")
                .bind(owner)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?,
            domains_verified: sqlx::query_scalar(
                "SELECT count(*) FROM ruphish_domains WHERE owner_id=? AND verified_until>?",
            )
            .bind(owner)
            .bind(now)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?,
        };

        // Attention items.
        let unverified_enabled_domains: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ruphish_domains WHERE owner_id=? AND enabled=1 AND verified_until<=?",
        )
        .bind(owner)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        let delivery_problems: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ruphish_delivery d JOIN campaigns c ON c.id=d.campaign_id \
             WHERE c.user_id=? AND c.status!='Completed' AND d.state IN ('failed','blocked','unknown')",
        )
        .bind(owner)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        let next_launch = sqlx::query(
            "SELECT id,name,launch_date FROM campaigns \
             WHERE user_id=? AND status!='Completed' AND unixepoch(launch_date)>? \
             ORDER BY unixepoch(launch_date),id LIMIT 1",
        )
        .bind(owner)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?;
        let next_launch = match next_launch {
            Some(r) => Some(NextLaunch {
                id: r.try_get("id").map_err(storage)?,
                name: r.try_get("name").map_err(storage)?,
                launch: r
                    .try_get::<Option<String>, _>("launch_date")
                    .map_err(storage)?
                    .unwrap_or_default(),
            }),
            None => None,
        };

        tx.commit().await.map_err(storage)?;
        Ok(Snapshot {
            as_of: now,
            campaigns,
            recent,
            inventory,
            attention: Attention {
                unverified_enabled_domains,
                delivery_problems,
                next_launch,
            },
        })
    }

    async fn posture(&self, caller: Identity, window: Window, now: i64) -> Result<Metrics, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        super::resources::authorize(&mut tx, caller, false).await?;
        let since = window.since(now);
        let counts = aggregate(&mut tx, caller.id, since, now, None).await?;
        let previous = if let Some(until) = since {
            let start = until - (now - until);
            Some(PreviousPeriod {
                since: start,
                until,
                counts: aggregate(&mut tx, caller.id, Some(start), until - 1, None).await?,
            })
        } else {
            None
        };
        tx.commit().await.map_err(storage)?;
        Ok(Metrics { counts, previous })
    }
}

/// Group by real campaign-recipient identity, excluding ambiguous historical
/// addresses exactly as per-campaign analysis does. Shared domain rules derive
/// progression from evidence without changing the recorded history.
async fn aggregate(
    connection: &mut sqlx::SqliteConnection,
    owner: i64,
    since: Option<i64>,
    now: i64,
    campaign: Option<i64>,
) -> Result<Posture, Error> {
    let flags = super::engagement::FLAGS;
    let groups = super::engagement::GROUPS;
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"
        WITH identities AS (
            SELECT r.campaign_id,r.email,min(r.id) AS id
            FROM results r JOIN campaigns c ON c.id=r.campaign_id
            WHERE c.user_id=?1 AND (?4 IS NULL OR c.id=?4)
            GROUP BY r.campaign_id,r.email HAVING count(*)=1
        ), accepted AS (
            SELECT i.*,min(unixepoch(e.time)) AS first_sent
            FROM identities i JOIN events e ON e.campaign_id=i.campaign_id AND e.email=i.email
            WHERE e.message='Email Sent' AND unixepoch(e.time)<=?3
            GROUP BY i.id
        ), cohort AS (
            SELECT * FROM accepted WHERE (?2 IS NULL OR first_sent>=?2)
        ), outcomes AS (
            SELECT c.id, {flags}
            FROM cohort c LEFT JOIN events e ON e.campaign_id=c.campaign_id AND e.email=c.email
                AND unixepoch(e.time)<=?3
            GROUP BY c.id
        )
        {groups}
    "#,
    )))
    .bind(owner)
    .bind(since)
    .bind(now)
    .bind(campaign)
    .fetch_all(connection)
    .await
    .map_err(storage)?;
    let summary = super::engagement::summarize(rows).map_err(storage)?;
    let [delivered, opened, clicked, _, reported] = summary.counts;
    Ok(Posture {
        delivered,
        opened,
        clicked,
        reported,
        credentials_entered: summary.credentials_entered,
    })
}
