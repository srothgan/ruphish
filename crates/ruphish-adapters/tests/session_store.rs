//! Persistence and revocation guarantees using real SQLite and library sessions.
#![allow(
    clippy::unwrap_used,
    reason = "Integration fixture setup and assertions"
)]
use ruphish_adapters::sqlite::{StartupDatabase, sessions::Sessions};
use std::sync::Arc;
use time::{Duration, OffsetDateTime};
use tower_sessions_core::{
    Session, SessionStore,
    session::{Id, Record},
};

#[tokio::test]
async fn late_session_write_cannot_restore_logout_across_independent_stores() {
    let temporary = tempfile::tempdir().unwrap();
    let database = StartupDatabase::open(&temporary.path().join("accounts.db"), "")
        .await
        .unwrap();
    let a = Arc::new(Sessions::new(database.pool()));
    let b = Arc::new(Sessions::new(database.pool()));
    let initial = Session::new(None, a.clone(), None);
    initial.insert("user", 42_i64).await.unwrap();
    initial.save().await.unwrap();
    let id = initial.id().unwrap();
    let overlapping = Session::new(Some(id), b.clone(), None);
    assert_eq!(overlapping.get::<i64>("user").await.unwrap(), Some(42));
    initial.flush().await.unwrap();
    overlapping.insert("flash", "saved").await.unwrap();
    assert!(overlapping.save().await.is_err());
    assert!(a.load(&id).await.unwrap().is_none());
    assert!(b.load(&id).await.unwrap().is_none());
    database.close().await;
}

#[tokio::test]
async fn collision_expiry_storage_failure_and_restart_preserve_session_boundaries() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("accounts.db");
    let database = StartupDatabase::open(&path, "").await.unwrap();
    let store = Sessions::new(database.pool());
    let key = store.cookie_key().await.unwrap();
    let mut original = Record {
        id: Id(7),
        data: Default::default(),
        expiry_date: OffsetDateTime::now_utc() + Duration::hours(1),
    };
    original.data.insert("user".into(), serde_json::json!(42));
    store.create(&mut original).await.unwrap();
    let mut collision = original.clone();
    collision.data.insert("user".into(), serde_json::json!(99));
    store.create(&mut collision).await.unwrap();
    assert_ne!(collision.id, original.id);
    assert_eq!(
        store.load(&original.id).await.unwrap().unwrap().data["user"],
        42
    );
    let mut expired = original.clone();
    expired.id = Id(8);
    expired.expiry_date = OffsetDateTime::now_utc() - Duration::seconds(1);
    store.create(&mut expired).await.unwrap();
    assert!(store.load(&expired.id).await.unwrap().is_none());
    assert!(store.save(&expired).await.is_err());
    store.cleanup().await.unwrap();
    drop(store);
    database.close().await;
    let restarted = StartupDatabase::open(&path, "").await.unwrap();
    let store = Sessions::new(restarted.pool());
    assert_eq!(key, store.cookie_key().await.unwrap());
    assert_eq!(
        store.load(&original.id).await.unwrap().unwrap().data["user"],
        42
    );
    sqlx::query("CREATE TRIGGER fail_session_write BEFORE UPDATE ON ruphish_sessions BEGIN SELECT RAISE(ABORT,'synthetic failure'); END").execute(&restarted.pool()).await.unwrap();
    assert!(store.save(&original).await.is_err());
    assert_eq!(
        store.load(&original.id).await.unwrap().unwrap().data["user"],
        42
    );
    drop(store);
    restarted.close().await;
}
