//! Shared dashboard aggregation over the existing database. Read-only, and
//! never reads entered participant values: credentials-entered counts only the
//! stored password-presence boolean.
use ruphish_application::{auth::Identity, overview::*, resources::Error};
use sqlx::{PgPool, Row};

/// Dashboard reads share the startup pool.
pub struct Overview {
    pool: PgPool,
}
impl Overview {
    /// Compose dashboard aggregation over the existing database.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
fn storage(error: sqlx::Error) -> Error {
    crate::database::postgres::resource_error(error)
}

#[async_trait::async_trait]
impl Repository for Overview {
    async fn snapshot(&self, caller: Identity, now: i64) -> Result<Snapshot, Error> {
        let mut tx = crate::database::postgres::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        crate::database::postgres::resources::authorize(&mut tx, caller, false).await?;

        // Recent campaigns, newest first, with audience size.
        let rows = sqlx::query(
            "SELECT id,name,status,launch_date,\
             (SELECT count(*) FROM results r WHERE r.campaign_id=c.id) AS total \
             FROM campaigns c  ORDER BY id DESC LIMIT 5",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let mut recent = Vec::with_capacity(rows.len());
        for r in rows {
            recent.push(RecentCampaign {
                id: r.try_get("id").map_err(storage)?,
                name: r.try_get("name").map_err(storage)?,
                status: ruphish_domain::campaign::CampaignStatus::from_stored(
                    &r.try_get::<String, _>("status").map_err(storage)?,
                )
                .ok_or(Error::Internal)?,
                launch: r
                    .try_get::<Option<String>, _>("launch_date")
                    .map_err(storage)?
                    .unwrap_or_default(),
                recipients: r.try_get("total").map_err(storage)?,
                outcomes: aggregate(&mut tx, None, now, Some(r.try_get("id").map_err(storage)?))
                    .await?,
            });
        }

        // Resource inventory. Each installation-wide count is an explicit literal query.
        let inventory = Inventory {
            reporting_mailboxes: sqlx::query_scalar(
                "SELECT count(*) FROM ruphish_mailboxes ",
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?,
            templates: sqlx::query_scalar(crate::database::postgres::inventory_sql::TEMPLATES)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?,
            pages: sqlx::query_scalar(crate::database::postgres::inventory_sql::PAGES)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?,
            groups: sqlx::query_scalar(crate::database::postgres::inventory_sql::GROUPS)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?,
            profiles: sqlx::query_scalar(crate::database::postgres::inventory_sql::PROFILES)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?,
            domains: sqlx::query_scalar("SELECT count(*) FROM ruphish_domains ")
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?,
            domains_verified: sqlx::query_scalar(
                "SELECT count(*) FROM ruphish_domains WHERE (SELECT verification_required FROM ruphish_domain_policy WHERE id=ruphish_domains.id) AND (SELECT eligible_until FROM ruphish_domain_policy WHERE id=ruphish_domains.id)>$1",
            )
            .bind(now)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?,
        };

        // Attention items.
        let unverified_enabled_domains: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ruphish_domains WHERE enabled=TRUE AND (SELECT verification_required FROM ruphish_domain_policy WHERE id=ruphish_domains.id) AND (SELECT eligible_until FROM ruphish_domain_policy WHERE id=ruphish_domains.id)<=$1",
        )
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        let delivery_problems: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ruphish_delivery d JOIN campaigns c ON c.id=d.campaign_id \
             WHERE c.status!='Completed' AND d.state IN ('failed','blocked','unknown')",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        let campaign_ids: Vec<i64> = sqlx::query_scalar("SELECT DISTINCT c.id FROM campaigns c JOIN ruphish_delivery d ON d.campaign_id=c.id WHERE c.status!='Completed' AND d.state IN ('failed','blocked','unknown') ORDER BY c.id LIMIT 10")
            .fetch_all(&mut *tx).await.map_err(storage)?;
        let next_launch = sqlx::query(
            "SELECT id,name,launch_date FROM campaigns \
             WHERE status!='Completed' AND (CASE WHEN launch_date>='1000' THEN floor(extract(epoch FROM launch_date::timestamptz))::bigint END)>$1 \
             ORDER BY (CASE WHEN launch_date>='1000' THEN floor(extract(epoch FROM launch_date::timestamptz))::bigint END),id LIMIT 1",
        )
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

        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::database::postgres::commit_error(e)))?;
        Ok(Snapshot {
            as_of: now,
            recent,
            inventory,
            attention: Attention {
                unverified_enabled_domains,
                delivery_problems,
                campaign_ids,
                next_launch,
            },
        })
    }

    async fn posture(&self, caller: Identity, window: Window, now: i64) -> Result<Metrics, Error> {
        let mut tx = crate::database::postgres::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        crate::database::postgres::resources::authorize(&mut tx, caller, false).await?;
        let since = window.since(now);
        let counts = aggregate(&mut tx, since, now, None).await?;
        let previous = if let Some(until) = since {
            let start = until - (now - until);
            Some(PreviousPeriod {
                since: start,
                until,
                counts: aggregate(&mut tx, Some(start), until - 1, None).await?,
            })
        } else {
            None
        };
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::database::postgres::commit_error(e)))?;
        Ok(Metrics { counts, previous })
    }
}

/// Group by real campaign-recipient identity, excluding ambiguous historical
/// addresses exactly as per-campaign analysis does. Shared domain rules derive
/// progression from evidence without changing the recorded history.
async fn aggregate(
    connection: &mut sqlx::PgConnection,
    since: Option<i64>,
    now: i64,
    campaign: Option<i64>,
) -> Result<Posture, Error> {
    let flags = crate::database::postgres::engagement::FLAGS;
    let groups = crate::database::postgres::engagement::GROUPS;
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"
        WITH parameters AS (SELECT $1::bigint AS since, $2::bigint AS now_time, $3::bigint AS campaign), identities AS (
            SELECT r.campaign_id,r.email,r.id
            FROM results r JOIN campaigns c ON c.id=r.campaign_id
            WHERE ((SELECT campaign FROM parameters) IS NULL OR c.id=(SELECT campaign FROM parameters))
        ), accepted AS (
            SELECT i.*,min((CASE WHEN e.time>='1000' THEN floor(extract(epoch FROM e.time::timestamptz))::bigint END)) AS first_sent
            FROM identities i JOIN event_history e ON e.campaign_id=i.campaign_id AND e.recipient_id=i.id
            WHERE e.message='Email Sent' AND (CASE WHEN e.time>='1000' THEN floor(extract(epoch FROM e.time::timestamptz))::bigint END)<=(SELECT now_time FROM parameters)
            GROUP BY i.id,i.campaign_id,i.email
        ), cohort AS (
            SELECT * FROM accepted WHERE ((SELECT since FROM parameters) IS NULL OR first_sent>=(SELECT since FROM parameters))
        ), outcomes AS (
            SELECT c.id, {flags}
            FROM cohort c LEFT JOIN event_history e ON e.campaign_id=c.campaign_id AND e.recipient_id=c.id
                AND (CASE WHEN e.time>='1000' THEN floor(extract(epoch FROM e.time::timestamptz))::bigint END)<=(SELECT now_time FROM parameters)
            GROUP BY c.id
        )
        {groups}
    "#,
    )))
    .bind(since)
    .bind(now)
    .bind(campaign)
    .fetch_all(connection)
    .await
    .map_err(storage)?;
    let summary = crate::database::postgres::engagement::summarize(rows).map_err(storage)?;
    let [delivered, opened, clicked, _, reported] = summary.counts;
    Ok(Posture {
        delivered,
        opened,
        clicked,
        reported,
        credentials_entered: summary.credentials_entered,
    })
}
