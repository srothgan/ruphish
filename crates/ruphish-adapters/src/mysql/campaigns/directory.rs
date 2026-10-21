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
        let mut tx = crate::mysql::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        super::super::resources::authorize(&mut tx, caller, false).await?;
        let limit = limit.clamp(1, 200);
        let rows = sqlx::query("SELECT 0 AS kind,id AS draft_id,NULL AS campaign_id,name,'Draft' AS status,NULL AS total,NULL AS launch_date,revision
FROM ruphish_campaign_drafts WHERE owner_id=? AND campaign_id IS NULL AND ?=1 AND ?=0 AND id>?
UNION ALL
SELECT 1 AS kind,'' AS draft_id,c.id AS campaign_id,c.name,COALESCE(c.status,'') AS status,(SELECT COUNT(*) FROM results r WHERE r.campaign_id=c.id) AS total,c.launch_date,NULL AS revision
FROM campaigns c WHERE c.user_id=? AND ?=1 AND c.id>? AND (?=0 OR (?=1 AND c.status='Completed') OR (?=2 AND (c.status IS NULL OR c.status<>'Completed')))
ORDER BY kind,draft_id,campaign_id LIMIT ?")
            .bind(caller.id).bind(i32::from(filter.drafts())).bind(after.kind).bind(&after.draft)
            .bind(caller.id).bind(i32::from(filter.executions())).bind(after.campaign)
            .bind(filter.completed()).bind(filter.completed()).bind(filter.completed()).bind(limit + 1)
            .fetch_all(&mut *tx).await.map_err(storage)?;
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
        Ok(Page::new(items, limit as usize))
    }
}
