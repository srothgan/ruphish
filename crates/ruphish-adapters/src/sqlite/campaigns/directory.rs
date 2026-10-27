//! Unified management projection; both families are read in one database snapshot.
use super::{Campaigns, storage};
use ruphish_application::{
    auth::Identity,
    campaign::directory::{Cursor, Entry, Filter, Page},
    resources::Error,
};
use sqlx::Row;
impl Campaigns {
    pub(super) async fn management_directory(
        &self,
        caller: Identity,
        filter: Filter,
        after: Cursor,
        limit: i64,
    ) -> Result<Page, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        super::super::resources::authorize(&mut tx, caller, false).await?;
        let limit = limit.clamp(1, 200);
        let rows = if let Some(order) = &after.sorted {
            let position = order.after.clone().unwrap_or_default();
            let sql = crate::campaign_sort::placeholders(
                format!(
                    "{} {}",
                    r#"WITH data AS (
SELECT 0 AS kind,id AS draft_id,CAST(0 AS BIGINT) AS campaign_id,name,'Draft' AS status,NULL AS total,NULL AS launch_date,revision
FROM ruphish_campaign_drafts WHERE owner_id=? AND campaign_id IS NULL AND ?=1
UNION ALL
SELECT 1 AS kind,'' AS draft_id,c.id AS campaign_id,c.name,COALESCE(c.status,'') AS status,(SELECT COUNT(*) FROM results r WHERE r.campaign_id=c.id) AS total,c.launch_date,NULL AS revision
FROM campaigns c WHERE c.user_id=? AND ?=1 AND (?=0 OR (?=1 AND c.status='Completed') OR (?=2 AND (c.status IS NULL OR c.status<>'Completed')))
)"#,
                    crate::campaign_sort::tail(
                        crate::campaign_sort::directory(order.sort, "sqlite"),
                        order.order,
                        "sqlite"
                    )
                ),
                "sqlite",
            );
            sqlx::query(sqlx::AssertSqlSafe(sql))
                .bind(caller.id)
                .bind(i32::from(filter.drafts()))
                .bind(caller.id)
                .bind(i32::from(filter.executions()))
                .bind(filter.completed())
                .bind(filter.completed())
                .bind(filter.completed())
                .bind(i32::from(order.after.is_some()))
                .bind(i32::from(position.present))
                .bind(&position.text)
                .bind(position.number)
                .bind(position.kind)
                .bind(&position.draft)
                .bind(position.id)
                .bind(limit + 1)
                .fetch_all(&mut *tx)
                .await
                .map_err(storage)?
        } else {
            sqlx::query("SELECT 0 AS kind,id AS draft_id,NULL AS campaign_id,name,'Draft' AS status,NULL AS total,NULL AS launch_date,revision
FROM ruphish_campaign_drafts WHERE owner_id=? AND campaign_id IS NULL AND ?=1 AND ?=0 AND id>?
UNION ALL
SELECT 1 AS kind,'' AS draft_id,c.id AS campaign_id,c.name,COALESCE(c.status,'') AS status,(SELECT COUNT(*) FROM results r WHERE r.campaign_id=c.id) AS total,c.launch_date,NULL AS revision
FROM campaigns c WHERE c.user_id=? AND ?=1 AND c.id>? AND (?=0 OR (?=1 AND c.status='Completed') OR (?=2 AND (c.status IS NULL OR c.status<>'Completed')))
ORDER BY kind,draft_id,campaign_id LIMIT ?")
            .bind(caller.id).bind(i32::from(filter.drafts())).bind(after.kind).bind(&after.draft)
            .bind(caller.id).bind(i32::from(filter.executions())).bind(after.campaign)
            .bind(filter.completed()).bind(filter.completed()).bind(filter.completed()).bind(limit + 1)
            .fetch_all(&mut *tx).await.map_err(storage)?
        };
        let sorted_next = if let Some(order) = &after.sorted {
            if rows.len() > limit as usize {
                let row = &rows[limit as usize - 1];
                let position = ruphish_domain::pagination::Position {
                    present: row.try_get::<i64, _>("sort_present").map_err(storage)? != 0,
                    text: row.try_get("sort_text").map_err(storage)?,
                    number: row.try_get("sort_number").map_err(storage)?,
                    kind: row.try_get("kind").map_err(storage)?,
                    draft: row.try_get("draft_id").map_err(storage)?,
                    id: row.try_get("campaign_id").map_err(storage)?,
                };
                Some(serde_json::to_string(&order.cursor(position)).map_err(|_| Error::Internal)?)
            } else {
                None
            }
        } else {
            None
        };
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let revision: Option<i64> = row.try_get("revision").map_err(storage)?;
            let id = if revision.is_some() {
                row.try_get("draft_id").map_err(storage)?
            } else {
                row.try_get::<i64, _>("campaign_id")
                    .map_err(storage)?
                    .to_string()
            };
            items.push(Entry {
                id,
                revision,
                name: row
                    .try_get::<Option<String>, _>("name")
                    .map_err(storage)?
                    .unwrap_or_default(),
                status: row.try_get("status").map_err(storage)?,
                total: row.try_get("total").map_err(storage)?,
                launch: row.try_get("launch_date").map_err(storage)?,
            });
        }
        let mut page = Page::new(items, limit as usize);
        if after.sorted.is_some() {
            page.next_after = sorted_next;
        }
        Ok(page)
    }
}
