//! A migrated installation must retain usable credentials, relationships, and stored content.

use ruphish_adapters::{
    password, sqlite,
    template::{self, Context},
};
use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};
use std::{fs, path::Path};

#[tokio::test]
async fn migrated_installation_can_authenticate_and_render_its_saved_campaign_for_the_same_recipient()
 {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/compatibility/20180830215615.sqlite3");
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("ruphish.db");
    let original = fs::read(&source).unwrap();
    sqlite::upgrade_copy(&source, &destination).await.unwrap();
    let mut connection =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&destination))
            .await
            .unwrap();
    let account=sqlx::query("SELECT u.id,u.hash,u.api_key,r.slug FROM users AS u JOIN roles AS r ON r.id=u.role_id WHERE username=?").bind("historical-admin").fetch_one(&mut connection).await.unwrap();
    assert!(password::verify("FixtureOnlyPassword42!", &account.get::<String, _>("hash")).unwrap());
    assert!(!password::verify("wrong password", &account.get::<String, _>("hash")).unwrap());
    assert_eq!(account.get::<String, _>("api_key"), "historical-key");
    assert_eq!(account.get::<String, _>("slug"), "admin");
    let row=sqlx::query("SELECT t.html,r.first_name,r.last_name,r.email,r.r_id FROM campaigns AS c JOIN templates AS t ON t.id=c.template_id AND t.user_id=c.user_id JOIN results AS r ON r.campaign_id=c.id AND r.user_id=c.user_id WHERE c.id=53 AND c.user_id=?").bind(account.get::<i64,_>("id")).fetch_one(&mut connection).await.unwrap();
    let context = Context {
        first_name: row.get("first_name"),
        last_name: row.get("last_name"),
        email: row.get("email"),
        rid: row.get("r_id"),
        ..Context::default()
    };
    let html: String = row.get("html");
    assert_eq!(
        template::render(&html, &context).unwrap(),
        "<table><tr><td>Ada</td></tr></table>"
    );
    assert_eq!(context.rid, "preserved-rid");
    let unchanged_hash: String = sqlx::query_scalar("SELECT hash FROM users WHERE id=17")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert_eq!(
        unchanged_hash,
        account.get::<String, _>("hash"),
        "verification must not rewrite the password hash"
    );
    connection.close().await.unwrap();
    assert_eq!(fs::read(&source).unwrap(), original);
}
