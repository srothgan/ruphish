//! Shared count semantics for provider resource metadata queries.
use ruphish_application::resources::Kind;

#[derive(Clone, Copy)]
pub(crate) enum Dialect {
    Sqlite,
    Mysql,
    Postgres,
}

/// One snapshot statement; identifiers come from each provider's closed kind map.
pub(crate) fn query(kind: Kind, table: &'static str, dialect: Dialect) -> String {
    let selection = match dialect {
        Dialect::Sqlite | Dialect::Mysql => "r.user_id=? AND (? IS NULL OR r.id=?)",
        Dialect::Postgres => "r.user_id=$1 AND ($2::BIGINT IS NULL OR r.id=$2)",
    };
    let total = total(kind, dialect);
    let total = match dialect {
        Dialect::Mysql => format!("CAST({total} AS SIGNED)"),
        _ => total,
    };
    format!(
        "SELECT r.id,r.name,r.modified_date,{total} AS total FROM {table} r WHERE {selection} ORDER BY r.id"
    )
}

/// Ownership selects resources, not references. Missing references count as zero.
fn total(kind: Kind, dialect: Dialect) -> String {
    let reference = match kind {
        Kind::Group => "g.group_id",
        Kind::Template => "c.template_id",
        Kind::Page => "c.page_id",
        Kind::Profile => "c.smtp_id",
        Kind::LearningPage => "p.learning_page_id",
    };
    let selected = match dialect {
        // PostgreSQL can estimate a correlated equality as matching the dominant
        // parent and scan all history when rare IDs vanish from sampled stats.
        // An equivalent closed ID range keeps these parameterized probes indexed
        // under that skew, without changing session-wide planner settings.
        Dialect::Postgres => format!("{reference} BETWEEN r.id AND r.id"),
        _ => format!("{reference}=r.id"),
    };
    match kind {
        Kind::Group => format!("(SELECT count(*) FROM group_targets g WHERE {selected})"),
        Kind::LearningPage => {
            // Fence the campaign lookup to each selected policy. A join can build
            // a hash over unrelated campaigns even after indexing the policies.
            // No campaign produces NULL (excluded); a NULL status still counts.
            format!(
                "(SELECT count(*) FROM ruphish_campaign_policy p WHERE {selected} AND (SELECT COALESCE(c.status,'') != 'Completed' FROM campaigns c WHERE c.id=p.campaign_id))"
            )
        }
        _ => format!(
            "(SELECT count(*) FROM campaigns c WHERE {selected} AND COALESCE(c.status,'') != 'Completed')"
        ),
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
             INSERT INTO ruphish_campaign_policy VALUES(1,10),(2,10),(3,10),(4,10),(5,11),(6,20),(7,10);",
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
                    "WITH selected_resources AS (SELECT id FROM resources WHERE user_id=? AND (? IS NULL OR id=?)) SELECT r.id AS resource_id,{} AS total FROM selected_resources r ORDER BY r.id",
                    total(kind, Dialect::Sqlite)
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
