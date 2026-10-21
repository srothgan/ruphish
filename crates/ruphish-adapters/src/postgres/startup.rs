use super::{migrations, storage_error};
use crate::database::{Error, PostgresConfig, PostgresTls};
use sqlx::{
    ConnectOptions, Connection, PgConnection, PgPool, Row,
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
};
use std::time::Duration;
pub(crate) struct StartupDatabase {
    pool: PgPool,
}

pub(super) fn options(config: &PostgresConfig) -> Result<PgConnectOptions, Error> {
    config.validate()?;
    // SQLx reads these settings before builder overrides; none may alter our policy.
    for name in ["PGOPTIONS", "PGSSLCERT", "PGSSLKEY", "PGSSLROOTCERT"] {
        if std::env::var_os(name).is_some() {
            return Err(Error::Invalid(format!(
                "Remove {name}; configure PostgreSQL explicitly in Ruphish"
            )));
        }
    }
    let mut options = PgConnectOptions::new_without_pgpass()
        .host(&config.host)
        .port(config.port)
        .database(&config.database)
        .username(&config.username)
        .password(&config.password_value()?)
        .application_name("ruphish")
        .ssl_mode(match config.tls {
            PostgresTls::VerifyIdentity => PgSslMode::VerifyFull,
            PostgresTls::Disabled => PgSslMode::Disable,
        })
        .options([("search_path", "ruphish,pg_catalog"), ("timezone", "UTC")])
        .disable_statement_logging();
    if let Some(ca) = &config.ca_path {
        options = options.ssl_root_cert(ca);
    }
    Ok(options)
}
async fn settings(c: &mut PgConnection, lock: u32, statement: u32) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT set_config('lock_timeout',$1,false),set_config('statement_timeout',$2,false),set_config('idle_in_transaction_session_timeout',$2,false)")
        .bind(format!("{lock}s")).bind(format!("{statement}s")).execute(c).await?;
    Ok(())
}
impl StartupDatabase {
    pub(crate) fn pool(&self) -> PgPool {
        self.pool.clone()
    }
    pub(crate) async fn open(config: &PostgresConfig) -> Result<Self, Error> {
        let options = options(config)?;
        let mut connection = tokio::time::timeout(
            Duration::from_secs(config.connect_timeout_seconds.into()),
            PgConnection::connect_with(&options),
        )
        .await
        .map_err(|_| Error::Busy)?
        .map_err(storage_error)?;
        settings(&mut connection, config.lock_timeout_seconds, 300)
            .await
            .map_err(storage_error)?;
        let result = migrations::prepare(&mut connection).await;
        let closed = connection.close().await.map_err(storage_error);
        result?;
        closed?;
        let lock = config.lock_timeout_seconds;
        let statement = config.statement_timeout_seconds;
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(Duration::from_secs(config.acquire_timeout_seconds.into()))
            .after_connect(move |c, _| Box::pin(async move { settings(c, lock, statement).await }))
            .connect_with(options)
            .await
            .map_err(storage_error)?;
        Ok(Self { pool })
    }
    pub async fn bootstrap(
        &self,
        password: Option<String>,
        token: Option<String>,
    ) -> Result<Option<String>, Error> {
        let mut transaction = crate::postgres::locks::write(&self.pool)
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
            sqlx::query("INSERT INTO users(username,hash,api_key,role_id,password_change_required,last_login,account_locked) VALUES('admin',$1,$2,$3,TRUE,'0001-01-01T00:00:00Z',FALSE)").bind(hash).bind(token).bind(role).execute(&mut *transaction).await.map_err(storage_error)?;
        } else if let Some(row) = existing.first() {
            let id: i64 = row.try_get("id").map_err(storage_error)?;
            sqlx::query("UPDATE users SET hash=$1,password_change_required=TRUE WHERE id=$2")
                .bind(hash)
                .bind(id)
                .execute(&mut *transaction)
                .await
                .map_err(storage_error)?;
            sqlx::query("INSERT INTO ruphish_auth_state(user_id,epoch) VALUES($1,1) ON CONFLICT (user_id) DO UPDATE SET epoch=ruphish_auth_state.epoch+1").bind(id).execute(&mut *transaction).await.map_err(storage_error)?;
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
