use super::storage_error;
use crate::database::{
    Error, MySqlConfig, MySqlTls,
    migrations::{MySqlInspection, MySqlSchema},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::{
    ConnectOptions, Connection, MySqlConnection, Row,
    mysql::{MySqlConnectOptions, MySqlSslMode},
};
use std::{collections::BTreeMap, time::Duration};

#[derive(Deserialize)]
struct LegacySchema {
    version: i64,
    tables: BTreeMap<String, BTreeMap<String, String>>,
}

pub(crate) async fn inspect(config: &MySqlConfig) -> Result<MySqlInspection, Error> {
    let options = options(config)?;
    let mut connection = tokio::time::timeout(
        Duration::from_secs(u64::from(config.connect_timeout_seconds)),
        MySqlConnection::connect_with(&options),
    )
    .await
    .map_err(|_| Error::Busy)?
    .map_err(storage_error)?;
    // Own a dedicated connection: cancellation closes its socket, releasing GET_LOCK.
    // A session-level lock must never be returned to a pool with a live owner.
    let result = tokio::time::timeout(
        Duration::from_secs(u64::from(config.lock_timeout_seconds) + 30),
        inspect_connection(&mut connection, config),
    )
    .await
    .map_err(|_| Error::Busy);
    let closed = connection.close().await.map_err(storage_error);
    let result = result??;
    closed?;
    Ok(result)
}

async fn inspect_connection(
    c: &mut MySqlConnection,
    config: &MySqlConfig,
) -> Result<MySqlInspection, Error> {
    let (server, lock) = acquire(c, config).await?;
    let schema = assess(c).await?;
    release(c, &lock).await?;
    Ok(MySqlInspection {
        server_version: server,
        schema,
    })
}

pub(super) async fn assess(c: &mut MySqlConnection) -> Result<MySqlSchema, Error> {
    let tables = sqlx::query("SELECT TABLE_NAME,ENGINE,TABLE_TYPE FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() ORDER BY TABLE_NAME LIMIT 129")
        .fetch_all(&mut *c).await.map_err(storage_error)?;
    let schema = if tables.is_empty() {
        MySqlSchema::Empty
    } else {
        if tables.len() > 128 {
            return Err(Error::UnsupportedSchema);
        }
        for row in &tables {
            if row
                .try_get::<String, _>("TABLE_TYPE")
                .map_err(storage_error)?
                != "BASE TABLE"
                || row
                    .try_get::<Option<String>, _>("ENGINE")
                    .map_err(storage_error)?
                    .as_deref()
                    != Some("InnoDB")
            {
                return Err(Error::UnsupportedSchema);
            }
        }
        let has_history = tables.iter().any(|r| {
            r.try_get::<String, _>("TABLE_NAME").ok().as_deref() == Some("goose_db_version")
        });
        if !has_history {
            return Err(Error::UnsupportedSchema);
        }
        let customized: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM information_schema.TRIGGERS WHERE TRIGGER_SCHEMA=DATABASE()) OR EXISTS(SELECT 1 FROM information_schema.REFERENTIAL_CONSTRAINTS WHERE CONSTRAINT_SCHEMA=DATABASE())")
            .fetch_one(&mut *c).await.map_err(storage_error)?;
        if customized {
            return Err(Error::UnsupportedSchema);
        }
        recognize(c, tables.len()).await?
    };
    Ok(schema)
}

pub(super) async fn release(c: &mut MySqlConnection, lock: &str) -> Result<(), Error> {
    let released: Option<i64> = sqlx::query_scalar("SELECT RELEASE_LOCK(?)")
        .bind(lock)
        .fetch_one(c)
        .await
        .map_err(storage_error)?;
    if released != Some(1) {
        return Err(Error::Unavailable);
    }
    Ok(())
}

fn supported_server(version: &str) -> bool {
    if version.to_ascii_lowercase().contains("mariadb") {
        return false;
    }
    let mut parts = version.split('.');
    matches!(
        (parts.next(), parts.next()),
        (Some("8"), Some("4")) | (Some("9"), Some("7"))
    )
}

async fn recognize(c: &mut MySqlConnection, table_count: usize) -> Result<MySqlSchema, Error> {
    let schemas: Vec<LegacySchema> = serde_json::from_str(include_str!("legacy-shapes.json"))
        .map_err(|_| Error::UnsupportedSchema)?;
    // Reject down/applied duplicates, gaps and unknown versions; never infer state
    // solely from MAX(version_id) or adopt populated tables as a fresh database.
    let history: Vec<(i64, bool)> =
        sqlx::query_as("SELECT version_id,is_applied FROM goose_db_version ORDER BY id LIMIT 31")
            .fetch_all(&mut *c)
            .await
            .map_err(storage_error)?;
    let applied: Vec<i64> = history
        .iter()
        .filter(|(v, _)| *v != 0)
        .map(|(v, _)| *v)
        .collect();
    if history.is_empty()
        || history.len() > schemas.len() + 1
        || history.iter().any(|(_, applied)| !applied)
        || history.iter().filter(|(v, _)| *v == 0).count() != 1
        || history.first().map(|(v, _)| *v) != Some(0)
        || applied.is_empty()
        || applied.iter().zip(&schemas).any(|(v, s)| *v != s.version)
    {
        return Err(Error::UnsupportedSchema);
    }
    let expected = schemas
        .get(applied.len() - 1)
        .ok_or(Error::UnsupportedSchema)?;
    let columns = sqlx::query("SELECT TABLE_NAME,COLUMN_NAME,DATA_TYPE FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME<>'goose_db_version' ORDER BY TABLE_NAME,ORDINAL_POSITION LIMIT 4097")
        .fetch_all(c).await.map_err(storage_error)?;
    if columns.len() > 4096 {
        return Err(Error::UnsupportedSchema);
    }
    let mut actual: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for column in columns {
        actual
            .entry(column.try_get("TABLE_NAME").map_err(storage_error)?)
            .or_default()
            .insert(
                column.try_get("COLUMN_NAME").map_err(storage_error)?,
                column.try_get("DATA_TYPE").map_err(storage_error)?,
            );
    }
    if table_count != expected.tables.len() + 1 || actual != expected.tables {
        return Err(Error::UnsupportedSchema);
    }
    Ok(MySqlSchema::Gophish {
        version: expected.version,
        requires_legacy_timezone: expected.version < 20170828220440,
    })
}

pub(super) fn options(config: &MySqlConfig) -> Result<MySqlConnectOptions, Error> {
    config.validate()?;
    let options = MySqlConnectOptions::new()
        .host(&config.host)
        .port(config.port)
        .database(&config.database)
        .username(&config.username)
        .password(&config.password_value()?)
        .ssl_mode(match config.tls {
            MySqlTls::VerifyIdentity => MySqlSslMode::VerifyIdentity,
            MySqlTls::Disabled => MySqlSslMode::Disabled,
        })
        .charset("utf8mb4")
        .collation("utf8mb4_0900_bin")
        .timezone(Some("+00:00".into()))
        .disable_statement_logging();
    let options = if let Some(ca) = &config.ca_path {
        options.ssl_ca(ca)
    } else {
        options
    };
    Ok(options)
}

pub(super) async fn acquire(
    c: &mut MySqlConnection,
    config: &MySqlConfig,
) -> Result<(String, String), Error> {
    let server: String = sqlx::query_scalar("SELECT VERSION()")
        .fetch_one(&mut *c)
        .await
        .map_err(storage_error)?;
    if !supported_server(&server) {
        return Err(Error::Invalid("MySQL server version is outside the supported LTS lines (8.4 and 9.7); MariaDB is not supported".into()));
    }
    let lock = format!(
        "ruphish:migrate:{}",
        &hex::encode(Sha256::digest(config.database.as_bytes()))[..40]
    );
    let acquired: Option<i64> = sqlx::query_scalar("SELECT GET_LOCK(?,?)")
        .bind(&lock)
        .bind(config.lock_timeout_seconds)
        .fetch_one(&mut *c)
        .await
        .map_err(storage_error)?;
    if acquired != Some(1) {
        return Err(Error::Busy);
    }
    Ok((server, lock))
}
