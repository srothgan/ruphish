//! Restore empty observation tables for older account-preference upgrade fixtures.
//! Definitions come from the immutable application baseline, not the new migration.
pub const HISTORY: &str = r#"
INSERT INTO campaigns(id,user_id,name,status) VALUES(7,1,'Historical observations','Completed');
INSERT INTO results(id,campaign_id,user_id,email,status) VALUES(11,7,1,'alice@example.test','Submitted Data'),(12,7,1,'bob@example.test','Email Opened'),(13,7,1,'bob@example.test','Email Opened');
INSERT INTO events(id,campaign_id,email,time,message,details) VALUES(40,7,'alice@example.test','2026-10-23T00:00:00Z','Submitted Data','{"password_present":true}'),(41,7,'missing@example.test','2026-10-23T00:00:01Z','Email Opened',''),(42,7,'bob@example.test','2026-10-23T00:00:02Z','Clicked Link','');
INSERT INTO ruphish_event_sources(event_id,source) VALUES(40,'public'),(41,'legacy');
INSERT INTO ruphish_submission_ids(result_id,event_id) VALUES(11,'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');
INSERT INTO pages(id,user_id,name,html,redirect_url,capture_credentials,capture_passwords) VALUES(8,1,'Historical page','<form><input type="password" name="password"></form>','',FALSE,FALSE);
INSERT INTO ruphish_campaign_policy(campaign_id,capture_credentials,capture_passwords,redirect_url) VALUES(7,FALSE,FALSE,'');
"#;

pub const READ_HISTORY: &str =
    "SELECT id,kind,recipient_id,source,metadata FROM events ORDER BY id";
pub type Observation = (i64, String, Option<i64>, String, String);
pub fn verify_history(rows: Vec<Observation>) {
    assert_eq!(
        rows,
        vec![
            (
                40,
                "form_submitted".into(),
                Some(11),
                "public".into(),
                "{\"password_present\":true}".into()
            ),
            (41, "email_opened".into(), None, "legacy".into(), "".into()),
            (42, "link_clicked".into(), None, "legacy".into(), "".into()),
        ]
    );
}

fn definitions(baseline: &str) -> String {
    let tables = [
        "events",
        "pages",
        "ruphish_campaign_policy",
        "ruphish_submission_ids",
        "ruphish_event_sources",
    ];
    let mut sql = String::from(
        "DROP TABLE ruphish_learning_pages; DROP VIEW ruphish_domain_policy; DROP TABLE ruphish_campaign_closures; DROP TABLE ruphish_settings; DROP VIEW event_history; DROP TABLE events; DROP TABLE pages; DROP TABLE ruphish_campaign_policy; DROP TABLE ruphish_event_receipts;",
    );
    for statement in baseline.split(';') {
        let start = statement
            .find("CREATE TABLE ")
            .or_else(|| statement.find("CREATE INDEX "));
        let Some(start) = start else { continue };
        let definition = &statement[start..];
        let table = if definition.starts_with("CREATE TABLE ") {
            definition.split_whitespace().nth(2)
        } else {
            definition
                .split_once(" ON ")
                .and_then(|(_, tail)| tail.split([' ', '(']).next())
        };
        if table.is_some_and(|table| tables.contains(&table.trim_matches('`'))) {
            sql.push_str(definition);
            sql.push(';');
        }
    }
    sql.push_str("DELETE FROM _sqlx_migrations WHERE version>=5;");
    sql
}

pub async fn mysql(c: &mut sqlx::MySqlConnection) {
    sqlx::raw_sql(super::performance_versions::MYSQL)
        .execute(&mut *c)
        .await
        .unwrap();
    let populated: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM events)+(SELECT count(*) FROM pages)+(SELECT count(*) FROM ruphish_campaign_policy)+(SELECT count(*) FROM ruphish_event_receipts)").fetch_one(&mut *c).await.unwrap();
    assert_eq!(
        populated, 0,
        "preference fixture must have no observation data"
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(definitions(include_str!(
        "../../migrations/mysql/1_application.sql"
    ))))
    .execute(c)
    .await
    .unwrap();
}

pub async fn postgres(c: &mut sqlx::PgConnection) {
    sqlx::raw_sql(super::performance_versions::POSTGRES)
        .execute(&mut *c)
        .await
        .unwrap();
    let populated: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM events)+(SELECT count(*) FROM pages)+(SELECT count(*) FROM ruphish_campaign_policy)+(SELECT count(*) FROM ruphish_event_receipts)").fetch_one(&mut *c).await.unwrap();
    assert_eq!(
        populated, 0,
        "preference fixture must have no observation data"
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(definitions(include_str!(
        "../../migrations/postgres/1_application.sql"
    ))))
    .execute(c)
    .await
    .unwrap();
}
