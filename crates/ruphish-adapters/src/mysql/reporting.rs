//! Observed-event analysis with a separate cumulative progression summary.
use ruphish_application::{auth::Identity, reporting::*, resources::Error};
use sqlx::{MySqlPool, Row};

/// Authorized reporting snapshot reads using the startup pool.
pub struct Reports {
    pool: MySqlPool,
}
impl Reports {
    /// Compose reporting over the existing database.
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}
fn storage(error: sqlx::Error) -> Error {
    crate::mysql::resource_error(error)
}
const CTE: &str = r#"WITH parameters AS (SELECT ? AS campaign, ? AS since, ? AS until_time, ? AS recipient, CAST(? AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_0900_bin AS event_kind, CAST(? AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_0900_bin AS search, CAST(? AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_0900_bin AS source_filter, CAST(? AS JSON) AS ids), audience AS (
 SELECT r.*,d.state AS delivery FROM results r LEFT JOIN ruphish_delivery d ON d.result_id=r.id
 WHERE r.campaign_id=(SELECT campaign FROM parameters) AND ((SELECT recipient FROM parameters) IS NULL OR r.id=(SELECT recipient FROM parameters)) AND ((SELECT search FROM parameters)='' OR instr(lower(CONCAT(r.email,' ',COALESCE(r.first_name,''),' ',COALESCE(r.last_name,''))),lower((SELECT search FROM parameters)))>0)
 AND (JSON_LENGTH((SELECT ids FROM parameters))=0 OR r.id IN (SELECT selected_ids.value FROM parameters p JOIN JSON_TABLE(p.ids,'$[*]' COLUMNS(value BIGINT PATH '$')) selected_ids ON TRUE))
), observed AS (
 SELECT e.* FROM event_history e
 LEFT JOIN audience a ON a.id=e.recipient_id
 WHERE e.campaign_id=(SELECT campaign FROM parameters) AND ((SELECT since FROM parameters) IS NULL OR (CASE WHEN e.time>='1000' THEN TIMESTAMPDIFF(SECOND,'1970-01-01 00:00:00',CAST(REPLACE(LEFT(e.time,19),'T',' ') AS DATETIME)) END)>=(SELECT since FROM parameters)) AND ((SELECT until_time FROM parameters) IS NULL OR (CASE WHEN e.time>='1000' THEN TIMESTAMPDIFF(SECOND,'1970-01-01 00:00:00',CAST(REPLACE(LEFT(e.time,19),'T',' ') AS DATETIME)) END)<(SELECT until_time FROM parameters))
 AND ((SELECT event_kind FROM parameters)='' OR e.message=(SELECT event_kind FROM parameters)) AND ((SELECT source_filter FROM parameters)='' OR e.source=(SELECT source_filter FROM parameters))
 AND (((SELECT recipient FROM parameters) IS NULL AND (SELECT search FROM parameters)='' AND JSON_LENGTH((SELECT ids FROM parameters))=0) OR a.id IS NOT NULL)
), selected AS (SELECT * FROM audience a WHERE ((SELECT event_kind FROM parameters)='' AND (SELECT source_filter FROM parameters)='') OR EXISTS(SELECT 1 FROM observed o WHERE o.recipient_id=a.id))
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
        {
            return Err(Error::Invalid("analysis_invalid_filter".into()));
        }
        let mut tx = crate::mysql::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
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
        let missing: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM JSON_TABLE(CAST(? AS JSON),'$[*]' COLUMNS(value BIGINT PATH '$')) s WHERE NOT EXISTS(SELECT 1 FROM results r WHERE r.id=s.value AND r.campaign_id=?))")
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
                    .bind(
                        f.event
                            .map(ruphish_domain::events::Kind::label)
                            .unwrap_or_default(),
                    )
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
                FROM selected) AS recipients, (SELECT min((CASE WHEN time>='1000' THEN TIMESTAMPDIFF(SECOND,'1970-01-01 00:00:00',CAST(REPLACE(LEFT(time,19),'T',' ') AS DATETIME)) END))
                FROM observed) AS first,(SELECT max((CASE WHEN time>='1000' THEN TIMESTAMPDIFF(SECOND,'1970-01-01 00:00:00',CAST(REPLACE(LEFT(time,19),'T',' ') AS DATETIME)) END))
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
                SELECT COALESCE((SELECT CAST(sum(length(CONCAT(COALESCE(email,''),COALESCE(time,''),COALESCE(message,''),COALESCE(details,'')))) AS SIGNED)
                FROM observed),0) + COALESCE((SELECT CAST(sum(length(CONCAT(COALESCE(email,''),COALESCE(first_name,''),COALESCE(last_name,''),COALESCE(position,''),COALESCE(send_date,''),COALESCE(modified_date,'')))) AS SIGNED)
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
        let event_order = crate::reporting::event_order(f, "mysql");
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
            });
        }
        let recipient_order = crate::reporting::recipient_order(f, "mysql");
        let rows=query!(format!(r#"
                SELECT * FROM (
                SELECT a.id,a.email,CONCAT(COALESCE(a.first_name,''),' ',COALESCE(a.last_name,'')) AS name,
                    a.delivery,a.reported,
                    COALESCE(a.first_name,'') AS first_name,COALESCE(a.last_name,'') AS last_name,
                    COALESCE(a.position,'') AS position,a.send_date,a.modified_date,
                    CAST(sum(CASE WHEN e.message='Email Sent' THEN 1 ELSE 0 END) AS SIGNED) AS accepted,
                    CAST(sum(CASE WHEN e.message='Email Opened' THEN 1 ELSE 0 END) AS SIGNED) AS opened,
                    CAST(sum(CASE WHEN e.message='Clicked Link' THEN 1 ELSE 0 END) AS SIGNED) AS clicked,
                    CAST(sum(CASE WHEN e.message='Submitted Data' THEN 1 ELSE 0 END) AS SIGNED) AS submitted,
                    CAST(sum(CASE WHEN e.message='Email Reported' THEN 1 ELSE 0 END) AS SIGNED) AS reports,
                    NOT EXISTS(SELECT 1
                FROM event_history h
                WHERE h.campaign_id=(SELECT campaign FROM parameters)
                AND h.recipient_id=a.id
                AND h.message='Email Reported') AS missing_report
                FROM selected a
                LEFT JOIN observed e ON e.recipient_id=a.id
                GROUP BY a.id,a.email,a.first_name,a.last_name,a.delivery,a.reported,a.position,a.send_date,a.modified_date
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
                SELECT ((CASE WHEN time>='1000' THEN TIMESTAMPDIFF(SECOND,'1970-01-01 00:00:00',CAST(REPLACE(LEFT(time,19),'T',' ') AS DATETIME)) END) DIV {width})*{width} AS bucket,message,count(*) AS total
                FROM observed
                WHERE (CASE WHEN time>='1000' THEN TIMESTAMPDIFF(SECOND,'1970-01-01 00:00:00',CAST(REPLACE(LEFT(time,19),'T',' ') AS DATETIME)) END) IS NOT NULL
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
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        Ok(result)
    }
}
