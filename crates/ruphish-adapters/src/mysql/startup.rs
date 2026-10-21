use super::{migrations, preflight, storage_error};
use crate::database::{Error, MySqlConfig};
use sqlx::{Connection, MySqlConnection, MySqlPool, Row, mysql::MySqlPoolOptions};
use std::time::Duration;
pub(crate) struct StartupDatabase {
    pool: MySqlPool,
    legacy_backup: bool,
}
impl StartupDatabase {
    pub(crate) fn has_recovery_backup(&self) -> bool {
        self.legacy_backup
    }
    pub(crate) fn pool(&self) -> MySqlPool {
        self.pool.clone()
    }
    pub(crate) async fn open(config: &MySqlConfig) -> Result<Self, Error> {
        let options = preflight::options(config)?;
        let mut connection = tokio::time::timeout(
            Duration::from_secs(config.connect_timeout_seconds.into()),
            MySqlConnection::connect_with(&options),
        )
        .await
        .map_err(|_| Error::Busy)?
        .map_err(storage_error)?;
        configure_session(&mut connection, config.lock_timeout_seconds)
            .await
            .map_err(storage_error)?;
        let result = migrations::prepare(&mut connection, config).await;
        let legacy_backup = if result.is_ok() {
            sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='_ruphish_upgrade')").fetch_one(&mut connection).await.map_err(storage_error)?
        } else {
            false
        };
        let closed = connection.close().await.map_err(storage_error);
        result?;
        closed?;
        let timeout = config.lock_timeout_seconds;
        let pool = MySqlPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(Duration::from_secs(config.acquire_timeout_seconds.into()))
            .after_connect(move |connection, _| {
                Box::pin(async move {
                    configure_session(connection, timeout).await?;
                    Ok(())
                })
            })
            .connect_with(options)
            .await
            .map_err(storage_error)?;
        Ok(Self {
            pool,
            legacy_backup,
        })
    }
    pub async fn bootstrap(
        &self,
        password: Option<String>,
        token: Option<String>,
    ) -> Result<Option<String>, Error> {
        let mut transaction = crate::mysql::locks::write(&self.pool)
            .await
            .map_err(storage_error)?;
        super::locks::guard(&mut transaction, "accounts")
            .await
            .map_err(storage_error)?;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
            .fetch_one(&mut *transaction)
            .await
            .map_err(storage_error)?;
        let existing = sqlx::query(
            "SELECT id,password_change_required FROM users WHERE username='admin' ORDER BY id",
        )
        .fetch_all(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if existing.len() > 1 {
            return Err(Error::Invalid(
                "Ambiguous default administrator records".into(),
            ));
        }
        let refresh = existing
            .first()
            .map(|row| row.try_get::<Option<bool>, _>("password_change_required"))
            .transpose()
            .map_err(storage_error)?
            .flatten()
            .unwrap_or(false);
        if count != 0 && !refresh {
            transaction.rollback().await.map_err(storage_error)?;
            return Ok(None);
        }
        let password = match password.filter(|value| !value.is_empty()) {
            Some(value) => value,
            None => random_hex(16)?,
        };
        if password.len() > 1024 {
            return Err(Error::Invalid(
                "Initial administrator password exceeds the supported length".into(),
            ));
        }
        let candidate = password.clone();
        let hash = tokio::task::spawn_blocking(move || crate::password_worker::hash(&candidate))
            .await
            .map_err(|_| Error::Invalid("Password hashing task failed".into()))?
            .map_err(|_| Error::Invalid("Password hashing failed".into()))?;
        if count == 0 {
            let role: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE slug='admin'")
                .fetch_one(&mut *transaction)
                .await
                .map_err(storage_error)?;
            let token = match token.filter(|value| !value.is_empty()) {
                Some(value) => value,
                None => random_hex(32)?,
            };
            sqlx::query("INSERT INTO users(username,hash,api_key,role_id,password_change_required,last_login,account_locked) VALUES('admin',?,?,?,1,'0001-01-01 00:00:00',0)").bind(hash).bind(token).bind(role).execute(&mut *transaction).await.map_err(storage_error)?;
        } else if let Some(row) = existing.first() {
            let id: i64 = row.try_get("id").map_err(storage_error)?;
            sqlx::query("UPDATE users SET hash=?,password_change_required=1 WHERE id=?")
                .bind(hash)
                .bind(id)
                .execute(&mut *transaction)
                .await
                .map_err(storage_error)?;
            sqlx::query("INSERT INTO ruphish_auth_state(user_id,epoch) VALUES(?,1) ON DUPLICATE KEY UPDATE epoch=epoch+1").bind(id).execute(&mut *transaction).await.map_err(storage_error)?;
        }
        transaction.commit().await.map_err(super::commit_error)?;
        Ok(Some(password))
    }

    /// Drain and close every database connection before process exit.
    pub async fn close(self) {
        self.pool.close().await;
    }
}

fn random_hex(length: usize) -> Result<String, Error> {
    let mut bytes = vec![0; length];
    getrandom::fill(&mut bytes).map_err(|_| Error::Invalid("OS randomness unavailable".into()))?;
    Ok(hex::encode(bytes))
}

/// Keep every connection independent of unsafe server-wide SQL mode defaults.
async fn configure_session(
    connection: &mut MySqlConnection,
    timeout: u32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "SET SESSION sql_mode=CONCAT_WS(',',NULLIF(@@SESSION.sql_mode,''),'STRICT_ALL_TABLES')",
    )
    .execute(&mut *connection)
    .await?;
    for statement in [
        "SET SESSION innodb_lock_wait_timeout=?",
        "SET SESSION lock_wait_timeout=?",
    ] {
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .bind(timeout)
            .execute(&mut *connection)
            .await?;
    }
    sqlx::query("SET SESSION max_execution_time=30000")
        .execute(connection)
        .await?;
    Ok(())
}
