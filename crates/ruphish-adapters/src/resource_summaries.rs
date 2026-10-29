//! Shared count semantics for provider resource metadata queries.
use ruphish_application::resources::Kind;

pub(crate) enum Dialect {
    Sqlite,
    Mysql,
    Postgres,
}

/// One snapshot statement; identifiers still come from each provider's closed kind map.
pub(crate) fn query(kind: Kind, table: &'static str, dialect: Dialect) -> String {
    let (selection, total) = match dialect {
        Dialect::Sqlite => ("r.user_id=? AND (? IS NULL OR r.id=?)", "COALESCE(counts.total,0)"),
        Dialect::Mysql => ("r.user_id=? AND (? IS NULL OR r.id=?)", "CAST(COALESCE(counts.total,0) AS SIGNED)"),
        Dialect::Postgres => ("r.user_id=$1 AND ($2::BIGINT IS NULL OR r.id=$2)", "CAST(COALESCE(counts.total,0) AS BIGINT)"),
    };
    let counts = counts(kind);
    format!("WITH selected_resources AS (SELECT r.id,r.name,r.modified_date FROM {table} r WHERE {selection}) SELECT r.id,r.name,r.modified_date,{total} AS total FROM selected_resources r LEFT JOIN ({counts}) counts ON counts.resource_id=r.id ORDER BY r.id")
}

/// Count only IDs in the provider's `selected_resources` CTE.
/// Resource ownership selects IDs; reference ownership does not change counts.
/// Providers own identifiers, binds and snapshots.
pub(crate) fn counts(kind: Kind) -> String {
    match kind {
        // Keep SQLite's membership lookups correlated to selected IDs. A grouped
        // join lets it scan every owner's membership index first.
        Kind::Group => "SELECT s.id AS resource_id,(SELECT count(*) FROM group_targets g WHERE g.group_id=s.id) AS total FROM selected_resources s".to_owned(),
        Kind::LearningPage => "SELECT p.learning_page_id AS resource_id,count(*) AS total FROM campaigns c JOIN ruphish_campaign_policy p ON p.campaign_id=c.id JOIN selected_resources s ON s.id=p.learning_page_id WHERE COALESCE(c.status,'') != 'Completed' GROUP BY p.learning_page_id".to_owned(),
        Kind::Template | Kind::Page | Kind::Profile => {
            let column = match kind {
                Kind::Template => "template_id",
                Kind::Page => "page_id",
                _ => "smtp_id",
            };
            format!("SELECT c.{column} AS resource_id,count(*) AS total FROM campaigns c JOIN selected_resources s ON s.id=c.{column} WHERE COALESCE(c.status,'') != 'Completed' GROUP BY c.{column}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Count/output semantics only. The SQLite repository test separately checks
    /// the actual execution plan and VM work as unrelated memberships grow.
    #[tokio::test]
    async fn resource_summaries_aggregate_only_selected_ids() -> Result<(), sqlx::Error> {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await?;
        sqlx::raw_sql(
            "CREATE TABLE resources(id INTEGER,user_id INTEGER);
             INSERT INTO resources VALUES(10,1),(11,1),(20,2);
             CREATE TABLE group_targets(group_id INTEGER,target_id INTEGER);
             INSERT INTO group_targets VALUES(10,1),(10,2),(11,3),(20,4);
             CREATE TABLE campaigns(id INTEGER,status TEXT,template_id INTEGER,page_id INTEGER,smtp_id INTEGER);
             INSERT INTO campaigns VALUES(1,'In progress',10,10,10),(2,'Completed',10,10,10),(3,NULL,10,10,10),(4,'',10,10,10),(5,'In progress',11,11,11),(6,'In progress',20,20,20);
             CREATE TABLE ruphish_campaign_policy(campaign_id INTEGER,learning_page_id INTEGER);
             INSERT INTO ruphish_campaign_policy VALUES(1,10),(2,10),(3,10),(4,10),(5,11),(6,20);",
        )
        .execute(&pool)
        .await?;
        for kind in [
            Kind::Group,
            Kind::Template,
            Kind::Page,
            Kind::Profile,
            Kind::LearningPage,
        ] {
            for (owner, id, selected) in [
                (1, None, vec![10, 11]),
                (1, Some(10), vec![10]),
                (1, Some(11), vec![11]),
                (1, Some(20), vec![]),
                (1, Some(999), vec![]),
                (2, None, vec![20]),
                (3, None, vec![]),
            ] {
                let statement = format!(
                    "WITH selected_resources AS (SELECT id FROM resources WHERE user_id=? AND (? IS NULL OR id=?)) {} ORDER BY resource_id",
                    counts(kind)
                );
                let actual: Vec<(i64, i64)> = sqlx::query_as(sqlx::AssertSqlSafe(statement))
                    .bind(owner)
                    .bind(id)
                    .bind(id)
                    .fetch_all(&pool)
                    .await?;
                let expected: Vec<_> = selected
                    .into_iter()
                    .map(|id| {
                        let total = if id == 10 {
                            if kind == Kind::Group { 2 } else { 3 }
                        } else {
                            1
                        };
                        (id, total)
                    })
                    .collect();
                assert_eq!(actual, expected, "{kind:?}, owner {owner}, id {id:?}");
            }
        }
        Ok(())
    }
}
