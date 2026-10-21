//! Observed-event analysis with a separate cumulative progression summary.
use ruphish_application::{auth::Identity, reporting::*, resources::Error};
use sqlx::{Row, SqlitePool};

/// Authorized reporting snapshot reads using the startup pool.
pub struct Reports {
    pool: SqlitePool,
}
impl Reports {
    /// Compose reporting over the existing database.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}
fn storage(error: sqlx::Error) -> Error {
    crate::sqlite::resource_error(error)
}
const CTE: &str = r#"WITH identities AS (
 SELECT email, CASE WHEN count(*)=1 THEN min(id) END AS id FROM results WHERE campaign_id=?1 GROUP BY email
), audience AS (
 SELECT r.*,COALESCE(d.state,'historical') AS delivery FROM results r LEFT JOIN ruphish_delivery d ON d.result_id=r.id
 WHERE r.campaign_id=?1 AND (?4 IS NULL OR r.id=?4) AND (?6='' OR instr(lower(r.email||' '||COALESCE(r.first_name,'')||' '||COALESCE(r.last_name,'')),lower(?6))>0)
 AND (?8='[]' OR r.id IN (SELECT value FROM json_each(?8)))
), observed AS (
 SELECT e.*,a.id AS recipient_id,COALESCE(s.source,'legacy') AS source FROM events e
 LEFT JOIN identities i ON i.email=e.email
 LEFT JOIN audience a ON a.id=i.id
 LEFT JOIN ruphish_event_sources s ON s.event_id=e.id
 WHERE e.campaign_id=?1 AND (?2 IS NULL OR unixepoch(e.time)>=?2) AND (?3 IS NULL OR unixepoch(e.time)<?3)
 AND (?5='' OR e.message=?5) AND (?7='' OR COALESCE(s.source,'legacy')=?7)
 AND ((?4 IS NULL AND ?6='' AND ?8='[]') OR a.id IS NOT NULL)
), selected AS (SELECT * FROM audience a WHERE (?5='' AND ?7='') OR EXISTS(SELECT 1 FROM observed o WHERE o.recipient_id=a.id))
"#;
const TYPES: [&str; 5] = [
    "Email Sent",
    "Email Opened",
    "Clicked Link",
    "Submitted Data",
    "Email Reported",
];
#[async_trait::async_trait]
impl Snapshots for Reports {
    async fn snapshot(
        &self,
        caller: Identity,
        id: i64,
        f: &Filter,
        export: bool,
    ) -> Result<Analysis, Error> {
        if !ruphish_domain::reporting::valid_window(f.from, f.until, f.limit)
            || f.offset > 1_000_000
            || f.search.len() > 256
            || f.recipients.len() > 200
            || f.recipients.iter().any(|id| *id <= 0)
            || (!f.event.is_empty()
                && !ruphish_domain::reporting::EVENT_TYPES.contains(&f.event.as_str()))
            || !["", "legacy", "application", "public", "mailbox"].contains(&f.source.as_str())
        {
            return Err(Error::Invalid("analysis_invalid_filter".into()));
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        super::resources::authorize(&mut tx, caller, false).await?;
        let campaign = sqlx::query("SELECT name,status FROM campaigns WHERE id=? AND user_id=?")
            .bind(id)
            .bind(caller.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
            .ok_or(Error::NotFound("Campaign"))?;
        if let Some(recipient) = f.recipient {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM results WHERE id=? AND campaign_id=?)",
            )
            .bind(recipient)
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
            if !exists {
                return Err(Error::NotFound("Recipient"));
            }
        }
        let selected_ids = serde_json::to_string(&f.recipients).map_err(|_| Error::Internal)?;
        let missing: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM json_each(?) s WHERE NOT EXISTS(SELECT 1 FROM results r WHERE r.id=s.value AND r.campaign_id=?))")
            .bind(&selected_ids).bind(id).fetch_one(&mut *tx).await.map_err(storage)?;
        if missing {
            return Err(Error::NotFound("Recipient"));
        }
        let audience: i64 = sqlx::query_scalar("SELECT count(*) FROM results WHERE campaign_id=?")
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        macro_rules! query {
            ($tail:expr) => {
                sqlx::query(sqlx::AssertSqlSafe(format!("{CTE} {}", $tail)))
                    .bind(id)
                    .bind(f.from)
                    .bind(f.until)
                    .bind(f.recipient)
                    .bind(&f.event)
                    .bind(&f.search)
                    .bind(&f.source)
                    .bind(&selected_ids)
            };
        }
        let metrics = query!(
            r#"
                SELECT message,count(*) AS total,count(DISTINCT recipient_id) AS unique_count
                FROM observed
                GROUP BY message
            "#
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let mut unique = [0; 5];
        let mut totals = [0; 5];
        for r in metrics {
            let kind: String = r.try_get("message").map_err(storage)?;
            if let Some(i) = TYPES.iter().position(|s| *s == kind) {
                unique[i] = r.try_get("unique_count").map_err(storage)?;
                totals[i] = r.try_get("total").map_err(storage)?;
            }
        }
        let flags = super::engagement::FLAGS;
        let groups = super::engagement::GROUPS;
        let evidence = query!(format!(
            ", outcomes AS (SELECT e.recipient_id,{flags} FROM observed e WHERE e.recipient_id IS NOT NULL GROUP BY e.recipient_id) {groups}"
        ))
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let progression = super::engagement::summarize(evidence).map_err(storage)?;
        let counts = query!(
            r#"
                SELECT (SELECT count(*)
                FROM observed) AS events,(SELECT count(*)
                FROM selected) AS recipients, (SELECT min(unixepoch(time))
                FROM observed) AS first,(SELECT max(unixepoch(time))
                FROM observed) AS last
            "#
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        let event_total: i64 = counts.try_get("events").map_err(storage)?;
        let recipient_total: i64 = counts.try_get("recipients").map_err(storage)?;
        if export && (event_total > 100_000 || recipient_total > 100_000) {
            return Err(Error::Invalid("export_too_large: narrow the time range or recipient filter (100000 rows per sheet maximum)".into()));
        }
        let size = query!(r#"
                SELECT COALESCE((SELECT sum(length(CAST(COALESCE(email,'')||COALESCE(time,'')||COALESCE(message,'')||COALESCE(details,'') AS BLOB)))
                FROM observed),0) + COALESCE((SELECT sum(length(CAST(COALESCE(email,'')||COALESCE(first_name,'')||COALESCE(last_name,'') AS BLOB)))
                FROM selected),0) AS bytes
            "#)
            .fetch_one(&mut *tx).await.map_err(storage)?.try_get::<i64,_>("bytes").map_err(storage)?;
        if size > 32 * 1024 * 1024 && export {
            return Err(Error::Invalid(
                "export_too_large: narrow the filters (32 MiB input limit)".into(),
            ));
        }
        let limit = if export { 100_000 } else { f.limit };
        let offset = if export { 0 } else { f.offset };
        let rows = query!(format!(
            r#"
                SELECT *
                FROM observed
                ORDER BY julianday(time) DESC,id DESC LIMIT {limit} OFFSET {offset}
            "#
        ))
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let mut events = Vec::with_capacity(rows.len());
        for r in rows {
            let details: String = r
                .try_get::<Option<String>, _>("details")
                .map_err(storage)?
                .unwrap_or_default();
            let safe: serde_json::Value =
                serde_json::from_str(&crate::participant_privacy::details(&details))
                    .unwrap_or_default();
            events.push(Event {
                id: r.try_get("id").map_err(storage)?,
                recipient_id: r.try_get("recipient_id").map_err(storage)?,
                email: r.try_get("email").map_err(storage)?,
                time: r.try_get("time").map_err(storage)?,
                kind: r.try_get("message").map_err(storage)?,
                source: r.try_get("source").map_err(storage)?,
                password_present: safe["password_present"].as_bool(),
            });
        }
        let rows=query!(format!(r#"
                SELECT a.id,a.email,COALESCE(a.first_name,'')||' '||COALESCE(a.last_name,'') AS name,
                    a.delivery,a.reported,
                    sum(CASE WHEN e.message='Email Sent' THEN 1 ELSE 0 END) AS accepted,
                    sum(CASE WHEN e.message='Email Opened' THEN 1 ELSE 0 END) AS opened,
                    sum(CASE WHEN e.message='Clicked Link' THEN 1 ELSE 0 END) AS clicked,
                    sum(CASE WHEN e.message='Submitted Data' THEN 1 ELSE 0 END) AS submitted,
                    sum(CASE WHEN e.message='Email Reported' THEN 1 ELSE 0 END) AS reports,
                    NOT EXISTS(SELECT 1
                FROM events h
                WHERE h.campaign_id=?1
                AND h.email=a.email
                AND h.message='Email Reported') AS missing_report
                FROM selected a
                LEFT JOIN observed e ON e.recipient_id=a.id
                GROUP BY a.id
                ORDER BY a.email,a.id LIMIT {limit} OFFSET {offset}
            "#)).fetch_all(&mut *tx).await.map_err(storage)?;
        let mut recipients = Vec::with_capacity(rows.len());
        for r in rows {
            recipients.push(Recipient {
                id: r.try_get("id").map_err(storage)?,
                email: r.try_get("email").map_err(storage)?,
                name: r
                    .try_get::<String, _>("name")
                    .map_err(storage)?
                    .trim()
                    .into(),
                delivery: r.try_get("delivery").map_err(storage)?,
                counts: [
                    r.try_get("accepted").map_err(storage)?,
                    r.try_get("opened").map_err(storage)?,
                    r.try_get("clicked").map_err(storage)?,
                    r.try_get("submitted").map_err(storage)?,
                    r.try_get("reports").map_err(storage)?,
                ],
                historical_reported: r.try_get::<bool, _>("reported").map_err(storage)?
                    && r.try_get::<bool, _>("missing_report").map_err(storage)?,
            });
        }
        let first = counts
            .try_get::<Option<i64>, _>("first")
            .map_err(storage)?
            .unwrap_or(0);
        let last = counts
            .try_get::<Option<i64>, _>("last")
            .map_err(storage)?
            .unwrap_or(first);
        let width = (((last - first).max(0) / 100 + 60) / 60).max(1) * 60;
        let rows = query!(format!(
            r#"
                SELECT (unixepoch(time)/{width})*{width} AS bucket,message,count(*) AS total
                FROM observed
                WHERE unixepoch(time) IS NOT NULL
                GROUP BY bucket,message
                ORDER BY bucket
            "#
        ))
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let mut buckets = std::collections::BTreeMap::<i64, [i64; 5]>::new();
        for r in rows {
            let kind: String = r.try_get("message").map_err(storage)?;
            if let Some(i) = TYPES.iter().position(|s| *s == kind) {
                buckets
                    .entry(r.try_get("bucket").map_err(storage)?)
                    .or_default()[i] = r.try_get("total").map_err(storage)?;
            }
        }
        // Fill quiet intervals inside the observed range. Lines must return to zero
        // rather than connecting distant observations across unrecorded activity.
        if let (Some(start), Some(end)) = (
            buckets.keys().next().copied(),
            buckets.keys().next_back().copied(),
        ) {
            let mut time = start;
            while time <= end {
                buckets.entry(time).or_default();
                time += width;
            }
        }
        let result = Analysis {
            id,
            name: campaign.try_get("name").map_err(storage)?,
            status: campaign.try_get("status").map_err(storage)?,
            as_of: time::OffsetDateTime::now_utc().unix_timestamp(),
            audience,
            unique,
            progression: progression.counts,
            totals,
            event_total,
            recipient_total,
            events,
            recipients,
            buckets: buckets
                .into_iter()
                .map(|(time, counts)| Bucket { time, counts })
                .collect(),
            bucket_seconds: width,
        };
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
}
