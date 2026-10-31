//! Observed-event analysis with a separate cumulative progression summary.
use crate::reporting::{MAX_EXPORT_BYTES, MAX_EXPORT_ROWS, METRIC_LABELS};
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
// Qualify recipients by event kind within the time/source scope, then retain all
// eligible event kinds for that selection. Without an event filter, preserve
// unattributed history and recipients without observations.
const CTE: &str = r#"WITH audience AS (
 SELECT r.*,d.state AS delivery FROM results r LEFT JOIN ruphish_delivery d ON d.result_id=r.id
 WHERE r.campaign_id=?1 AND (?4 IS NULL OR r.id=?4) AND (?6='' OR instr(lower(r.email||' '||COALESCE(r.first_name,'')||' '||COALESCE(r.last_name,'')),lower(?6))>0)
 AND (?8='[]' OR r.id IN (SELECT value FROM json_each(?8)))
), eligible AS (
 SELECT e.* FROM event_history e
 LEFT JOIN audience a ON a.id=e.recipient_id
 WHERE e.campaign_id=?1 AND (?2 IS NULL OR unixepoch(e.time)>=?2) AND (?3 IS NULL OR unixepoch(e.time)<?3)
 AND (?7='' OR e.source=?7)
 AND ((?4 IS NULL AND ?6='' AND ?8='[]') OR a.id IS NOT NULL)
), selected AS (SELECT * FROM audience a WHERE (?5='[]' AND ?7='') OR EXISTS(SELECT 1 FROM eligible e WHERE e.recipient_id=a.id AND (?5='[]' OR e.message IN (SELECT value FROM json_each(?5)))))
, observed AS (SELECT e.* FROM eligible e WHERE ?5='[]' OR EXISTS(SELECT 1 FROM selected s WHERE s.id=e.recipient_id))
"#;
#[async_trait::async_trait]
impl Snapshots for Reports {
    async fn snapshot(
        &self,
        caller: Identity,
        id: i64,
        f: &Filter,
        export: bool,
    ) -> Result<Analysis, Error> {
        f.validate()?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        super::resources::authorize(&mut tx, caller, false).await?;
        let campaign = sqlx::query("SELECT name,status,COALESCE((SELECT track_opens FROM ruphish_campaign_policy WHERE campaign_id=campaigns.id),1) AS track_opens FROM campaigns WHERE id=? AND user_id=?")
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
        let event_kinds =
            serde_json::to_string(&f.event.iter().map(|kind| kind.label()).collect::<Vec<_>>())
                .map_err(|_| Error::Internal)?;
        macro_rules! query {
            ($tail:expr) => {
                sqlx::query(sqlx::AssertSqlSafe(format!("{CTE} {}", $tail)))
                    .bind(id)
                    .bind(f.from)
                    .bind(f.until)
                    .bind(f.recipient)
                    .bind(&event_kinds)
                    .bind(&f.search)
                    .bind(
                        f.source
                            .map(ruphish_domain::reporting::Source::as_str)
                            .unwrap_or_default(),
                    )
                    .bind(&selected_ids)
            };
        }
        let metrics = query!(
            r#"
                SELECT message,count(*) AS total,count(DISTINCT recipient_id) AS unique_count, min(unixepoch(time)) AS first_observed, max(unixepoch(time)) AS last_observed
                FROM observed
                GROUP BY message
            "#
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let mut first_observed = [None; 5];
        let mut last_observed = [None; 5];
        let mut unique = [0; 5];
        let mut totals = [0; 5];
        for r in metrics {
            let kind: String = r.try_get("message").map_err(storage)?;
            if let Some(i) = METRIC_LABELS.iter().position(|s| *s == kind) {
                first_observed[i] = r.try_get("first_observed").map_err(storage)?;
                last_observed[i] = r.try_get("last_observed").map_err(storage)?;
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
        if export
            && (event_total > MAX_EXPORT_ROWS as i64 || recipient_total > MAX_EXPORT_ROWS as i64)
        {
            return Err(Error::Invalid("export_too_large: narrow the time range or recipient filter (100000 rows per sheet maximum)".into()));
        }
        if export {
            let size = query!(r#"
                    SELECT COALESCE((SELECT sum(length(CAST(COALESCE(email,'')||COALESCE(time,'')||COALESCE(message,'')||COALESCE(details,'') AS BLOB)))
                    FROM observed),0) + COALESCE((SELECT sum(length(CAST(COALESCE(email,'')||COALESCE(first_name,'')||COALESCE(last_name,'')||COALESCE(position,'')||COALESCE(send_date,'')||COALESCE(modified_date,'') AS BLOB)))
                    FROM selected),0) AS bytes
                "#)
                .fetch_one(&mut *tx).await.map_err(storage)?.try_get::<i64,_>("bytes").map_err(storage)?;
            if size > MAX_EXPORT_BYTES {
                return Err(Error::Invalid(
                    "export_too_large: narrow the filters (32 MiB input limit)".into(),
                ));
            }
        }
        let limit = if export { MAX_EXPORT_ROWS } else { f.limit };
        let offset = if export { 0 } else { f.offset };
        let event_order = crate::reporting::event_order(f, "sqlite");
        let rows = query!(format!(
            r#"
                SELECT *
                FROM observed
                ORDER BY {event_order} LIMIT {limit} OFFSET {offset}
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
                client: crate::participant_privacy::client(
                    r.try_get::<Option<String>, _>("client")
                        .map_err(storage)?
                        .as_deref(),
                )?,
                edited: r.try_get("edited").map_err(storage)?,
                id: r.try_get("id").map_err(storage)?,
                recipient_id: r.try_get("recipient_id").map_err(storage)?,
                email: r.try_get("email").map_err(storage)?,
                time: r.try_get("time").map_err(storage)?,
                kind: ruphish_domain::events::RecordedKind::from_label(
                    r.try_get("message").map_err(storage)?,
                ),
                source: ruphish_domain::reporting::Source::from_stored(
                    &r.try_get::<String, _>("source").map_err(storage)?,
                )
                .ok_or(Error::Internal)?,
                password_present: safe["password_present"].as_bool(),
                username_present: safe["username_present"].as_bool(),
            });
        }
        let recipient_order = crate::reporting::recipient_order(f, "sqlite");
        let rows=query!(format!(r#"
                SELECT * FROM (
                SELECT a.id,a.email,COALESCE(a.first_name,'')||' '||COALESCE(a.last_name,'') AS name,
                    a.delivery,a.reported,
                    COALESCE(a.first_name,'') AS first_name,COALESCE(a.last_name,'') AS last_name,
                    COALESCE(a.position,'') AS position,a.send_date,a.modified_date,
                    sum(CASE WHEN e.message='Email Sent' THEN 1 ELSE 0 END) AS accepted,
                    sum(CASE WHEN e.message='Email Opened' THEN 1 ELSE 0 END) AS opened,
                    sum(CASE WHEN e.message='Clicked Link' THEN 1 ELSE 0 END) AS clicked,
                    sum(CASE WHEN e.message='Submitted Data' THEN 1 ELSE 0 END) AS submitted,
                    sum(CASE WHEN e.message='Email Reported' THEN 1 ELSE 0 END) AS reports,
                    NOT EXISTS(SELECT 1
                FROM event_history h
                WHERE h.campaign_id=?1
                AND h.recipient_id=a.id
                AND h.message='Email Reported') AS missing_report
                FROM selected a
                LEFT JOIN observed e ON e.recipient_id=a.id
                GROUP BY a.id
                ) recipient_rows ORDER BY {recipient_order} LIMIT {limit} OFFSET {offset}
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
                delivery: r
                    .try_get::<Option<String>, _>("delivery")
                    .map_err(storage)?
                    .map(|state| {
                        ruphish_domain::campaign::DeliveryState::from_stored(&state)
                            .ok_or(Error::Internal)
                    })
                    .transpose()?,
                first_name: r.try_get("first_name").map_err(storage)?,
                last_name: r.try_get("last_name").map_err(storage)?,
                position: r.try_get("position").map_err(storage)?,
                send_date: r.try_get("send_date").map_err(storage)?,
                modified_date: r.try_get("modified_date").map_err(storage)?,
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
            if let Some(i) = METRIC_LABELS.iter().position(|s| *s == kind) {
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
        // Deduplicate within the filtered snapshot before pagination or time bucketing.
        let participant_rows = query!(format!(
            r#"
            , first_actions AS (
                SELECT recipient_id,message,min(unixepoch(time)) AS first_time FROM observed
                WHERE recipient_id IS NOT NULL AND unixepoch(time) IS NOT NULL
                GROUP BY recipient_id,message
            )
            SELECT (first_time / {width})*{width} AS bucket,message,count(*) AS total
            FROM first_actions GROUP BY bucket,message ORDER BY bucket
        "#
        ))
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let mut participant_increments = std::collections::BTreeMap::<i64, [i64; 5]>::new();
        for row in participant_rows {
            let kind: String = row.try_get("message").map_err(storage)?;
            if let Some(i) = METRIC_LABELS.iter().position(|s| *s == kind) {
                participant_increments
                    .entry(row.try_get("bucket").map_err(storage)?)
                    .or_default()[i] = row.try_get("total").map_err(storage)?;
            }
        }
        let participant_buckets =
            crate::reporting::participant_buckets(participant_increments, last, width);
        let result = Analysis {
            track_opens: campaign.try_get("track_opens").map_err(storage)?,
            participant_buckets,
            first_observed,
            last_observed,
            id,
            name: campaign.try_get("name").map_err(storage)?,
            status: ruphish_domain::campaign::CampaignStatus::from_stored(
                &campaign.try_get::<String, _>("status").map_err(storage)?,
            )
            .ok_or(Error::Internal)?,
            as_of: ruphish_application::datetime::now().as_second(),
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
