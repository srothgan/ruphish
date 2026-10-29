//! Dedicated startup connection: bounded ownership and transactional migrations.
use super::storage_error;
use crate::database::Error;
use sqlx::{
    PgConnection, SqlSafeStr,
    migrate::{Migrate, Migration, MigrationType, Migrator},
};
use std::borrow::Cow;
const BASELINE: &str = include_str!("../../migrations/postgres/1_application.sql");
fn migrator() -> Migrator {
    Migrator {
        migrations: Cow::Owned(vec![
            Migration::new(
                1,
                Cow::Borrowed("application"),
                MigrationType::Simple,
                BASELINE.into_sql_str(),
                false,
            ),
            Migration::new(
                2,
                Cow::Borrowed("user preferences"),
                MigrationType::Simple,
                include_str!("../../migrations/postgres/2_user_preferences.sql").into_sql_str(),
                false,
            ),
            Migration::new(
                3,
                Cow::Borrowed("UTC offset preference"),
                MigrationType::Simple,
                include_str!("../../migrations/postgres/3_utc_offset.sql").into_sql_str(),
                false,
            ),
            Migration::new(
                4,
                Cow::Borrowed("campaign drafts"),
                MigrationType::Simple,
                include_str!("../../migrations/postgres/4_campaign_drafts.sql").into_sql_str(),
                false,
            ),
            Migration::new(
                5,
                Cow::Borrowed("observations"),
                MigrationType::Simple,
                include_str!("../../migrations/postgres/5_observations.sql").into_sql_str(),
                false,
            ),
            Migration::new(
                6,
                Cow::Borrowed("company settings"),
                MigrationType::Simple,
                include_str!("../../migrations/postgres/6_settings.sql").into_sql_str(),
                false,
            ),
            Migration::new(
                7,
                Cow::Borrowed("learning pages"),
                MigrationType::Simple,
                include_str!("../../migrations/postgres/7_learning_pages.sql").into_sql_str(),
                false,
            ),
            Migration::new(
                8,
                Cow::Borrowed("worker recovery indexes"),
                MigrationType::Simple,
                include_str!("../../migrations/postgres/8_worker_recovery.sql").into_sql_str(),
                false,
            ),
            Migration::new(
                9,
                Cow::Borrowed("campaign fairness index"),
                MigrationType::Simple,
                include_str!("../../migrations/postgres/9_campaign_fairness.sql").into_sql_str(),
                false,
            ),
        ]),
        locking: false,
        ..Migrator::DEFAULT
    }
}
pub(super) async fn prepare(c: &mut PgConnection) -> Result<(), Error> {
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&mut *c)
        .await
        .map_err(storage_error)?;
    let version: u32 = version.parse().map_err(|_| Error::UnsupportedSchema)?;
    if !(170000..190000).contains(&version) {
        return Err(Error::Invalid(
            "PostgreSQL 17 and 18 are supported; other major versions are unverified".into(),
        ));
    }
    let encoding: String = sqlx::query_scalar("SHOW server_encoding")
        .fetch_one(&mut *c)
        .await
        .map_err(storage_error)?;
    let readonly: String = sqlx::query_scalar("SHOW transaction_read_only")
        .fetch_one(&mut *c)
        .await
        .map_err(storage_error)?;
    if encoding != "UTF8" || readonly != "off" {
        return Err(Error::Invalid(
            "PostgreSQL requires a writable UTF8 database".into(),
        ));
    }
    // Use SQLx's database-scoped lock, covering recognition as well as migration.
    c.lock().await.map_err(|_| Error::Busy)?;
    let schema: Option<String> =
        sqlx::query_scalar("SELECT nspname FROM pg_namespace WHERE nspname='ruphish'")
            .fetch_optional(&mut *c)
            .await
            .map_err(storage_error)?;
    if schema.is_none() {
        let occupied: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace WHERE n.nspname NOT IN ('pg_catalog','information_schema') AND n.nspname NOT LIKE 'pg_toast%' AND t.relkind IN ('r','p','v','m','f'))").fetch_one(&mut *c).await.map_err(storage_error)?;
        if occupied {
            return Err(Error::UnsupportedSchema);
        }
        sqlx::query("CREATE SCHEMA ruphish")
            .execute(&mut *c)
            .await
            .map_err(storage_error)?;
    }
    let owner: bool = sqlx::query_scalar(
        "SELECT pg_get_userbyid(nspowner)=current_user FROM pg_namespace WHERE nspname='ruphish'",
    )
    .fetch_one(&mut *c)
    .await
    .map_err(storage_error)?;
    if !owner {
        return Err(Error::Invalid(
            "PostgreSQL service role must own the ruphish schema for automatic migrations".into(),
        ));
    }
    // Remove the default public grant; only the owner controls this schema.
    sqlx::query("REVOKE ALL ON SCHEMA ruphish FROM PUBLIC")
        .execute(&mut *c)
        .await
        .map_err(storage_error)?;
    let tables: Vec<String> = sqlx::query_scalar("SELECT relname FROM pg_class WHERE relnamespace='ruphish'::regnamespace AND relkind IN ('r','p','v','m','f') ORDER BY relname").fetch_all(&mut *c).await.map_err(storage_error)?;
    if !tables.is_empty() && !tables.iter().any(|t| t == "_sqlx_migrations") {
        return Err(Error::UnsupportedSchema);
    }
    migrator()
        .run_direct(None, &mut *c, false)
        .await
        .map_err(|_| Error::UnsupportedSchema)?;
    // Explicit schema/constraint inventory, checked independently of migration history.
    verify(c).await?;
    c.unlock().await.map_err(|_| Error::Unavailable)?;
    Ok(())
}
// PostgreSQL 18 adds NOT NULL catalog constraints; column nullability checks them
// on both supported majors, without depending on their generated constraint names.
const INVENTORY: &str = r#"
SELECT jsonb_build_object(
 'columns', (SELECT jsonb_agg(jsonb_build_array(table_name,column_name,data_type,is_nullable,character_maximum_length,is_identity,column_default,collation_name) ORDER BY table_name,ordinal_position) FROM information_schema.columns WHERE table_schema='ruphish' AND table_name<>'_sqlx_migrations'),
 'indexes', (SELECT jsonb_agg(jsonb_build_array(t.relname,ic.relname,i.indisunique,i.indisprimary,i.indisvalid,pg_get_indexdef(i.indexrelid)) ORDER BY t.relname,ic.relname) FROM pg_index i JOIN pg_class t ON t.oid=i.indrelid JOIN pg_class ic ON ic.oid=i.indexrelid WHERE t.relnamespace='ruphish'::regnamespace AND t.relname<>'_sqlx_migrations'),
 'constraints', (SELECT jsonb_agg(jsonb_build_array(t.relname,c.conname,c.contype,c.convalidated,COALESCE((to_jsonb(c)->>'conenforced')::boolean,true),pg_get_constraintdef(c.oid)) ORDER BY t.relname,c.conname) FROM pg_constraint c JOIN pg_class t ON t.oid=c.conrelid WHERE t.relnamespace='ruphish'::regnamespace AND t.relname<>'_sqlx_migrations' AND c.contype<>'n'),
 'triggers', (SELECT count(*) FROM pg_trigger g JOIN pg_class t ON t.oid=g.tgrelid WHERE t.relnamespace='ruphish'::regnamespace AND NOT g.tgisinternal),
 'relations', (SELECT jsonb_agg(jsonb_build_array(relname,relkind) ORDER BY relname) FROM pg_class WHERE relnamespace='ruphish'::regnamespace AND relkind IN ('r','p','v','m','f') AND relname<>'_sqlx_migrations')
)::text
"#;
async fn verify(c: &mut PgConnection) -> Result<(), Error> {
    let policy_definition: String =
        sqlx::query_scalar("SELECT pg_get_viewdef('ruphish.ruphish_domain_policy'::regclass)")
            .fetch_one(&mut *c)
            .await
            .map_err(storage_error)?;
    if !policy_definition
        .trim()
        .lines()
        .eq(include_str!("settings_view.sql").trim().lines())
    {
        return Err(Error::UnsupportedSchema);
    }

    let definition: String =
        sqlx::query_scalar("SELECT pg_get_viewdef('ruphish.event_history'::regclass)")
            .fetch_one(&mut *c)
            .await
            .map_err(storage_error)?;
    if !crate::observation_schema::matches(
        &definition,
        include_str!("../../migrations/postgres/5_observations.sql"),
        "ruphish",
    ) {
        return Err(Error::UnsupportedSchema);
    }
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("schema.json")).map_err(|_| Error::UnsupportedSchema)?;
    let actual: String = sqlx::query_scalar(INVENTORY)
        .fetch_one(&mut *c)
        .await
        .map_err(storage_error)?;
    let actual: serde_json::Value =
        serde_json::from_str(&actual).map_err(|_| Error::UnsupportedSchema)?;
    if actual != expected {
        return Err(Error::UnsupportedSchema);
    }
    Ok(())
}
