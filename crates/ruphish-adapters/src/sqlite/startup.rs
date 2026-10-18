use super::{MIGRATIONS, UpgradeError, apply_pending, connect, inspect};
use sqlx::{
    Connection, Row, SqliteConnection, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{
    fs::{File, OpenOptions, TryLockError},
    path::{Path, PathBuf},
    time::Duration,
};

/// Failure to prepare a database, recoverable backup or initial administrator.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    /// A database is unrecognized or its migration failed.
    #[error(transparent)]
    Upgrade(#[from] UpgradeError),
    /// A filesystem operation failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// A database operation failed.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    /// Bootstrap input, cryptography or migration configuration is invalid.
    #[error("{0}")]
    Invalid(String),
}

/// One owned SQLite pool and the recoverable backup created by this startup.
pub struct StartupDatabase {
    pool: SqlitePool,
    backup: Option<PathBuf>,
}

impl StartupDatabase {
    /// Share the owned pool with adapters; shutdown still belongs to the executable.
    pub fn pool(&self) -> SqlitePool {
        self.pool.clone()
    }
    /// Open a current database or initialize/upgrade it with no-clobber backup publication.
    ///
    /// All old application instances must be stopped before a schema upgrade. The
    /// short filesystem lock serializes cooperating startup operations, not Go writers.
    pub async fn open(path: &Path, migrations_prefix: &str) -> Result<Self, StartupError> {
        verify_migrations(migrations_prefix)?;
        let path = resolved(path)?;
        let _startup_lock = startup_lock(&path).await?;
        let fresh = !path.try_exists()?;
        if fresh {
            super::create(&path).await?;
        }
        let mut connection = connect(&path, false).await?;
        if fresh {
            let mut transaction = connection.begin_with("BEGIN IMMEDIATE").await?;
            super::auth_schema::migrate(&mut transaction).await?;
            transaction.commit().await?;
        }
        let prepared = prepare(&mut connection, &path).await;
        let closed = connection.close().await;
        let backup = prepared?;
        closed?;
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(false)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options)
            .await?;
        Ok(Self { pool, backup })
    }

    /// Recoverable pre-upgrade snapshot, if this startup migrated the schema.
    pub fn backup_path(&self) -> Option<&Path> {
        self.backup.as_deref()
    }

    /// Atomically create the initial administrator or refresh its temporary password.
    ///
    /// The optional returned password is for a controlled console output, never ordinary logs.
    /// Existing API tokens and established password hashes are not replaced.
    pub async fn bootstrap(
        &self,
        password: Option<String>,
        token: Option<String>,
    ) -> Result<Option<String>, StartupError> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
            .fetch_one(&mut *transaction)
            .await?;
        let existing = sqlx::query(
            "SELECT id,password_change_required FROM users WHERE username='admin' ORDER BY id",
        )
        .fetch_all(&mut *transaction)
        .await?;
        if existing.len() > 1 {
            return Err(StartupError::Invalid(
                "Ambiguous default administrator records".into(),
            ));
        }
        let refresh = existing
            .first()
            .map(|row| row.try_get::<Option<bool>, _>("password_change_required"))
            .transpose()?
            .flatten()
            .unwrap_or(false);
        if count != 0 && !refresh {
            transaction.rollback().await?;
            return Ok(None);
        }
        let password = match password.filter(|value| !value.is_empty()) {
            Some(value) => value,
            None => random_hex(16)?,
        };
        if password.len() > 1024 {
            return Err(StartupError::Invalid(
                "Initial administrator password exceeds the supported length".into(),
            ));
        }
        let candidate = password.clone();
        let hash = tokio::task::spawn_blocking(move || crate::password_worker::hash(&candidate))
            .await
            .map_err(|_| StartupError::Invalid("Password hashing task failed".into()))?
            .map_err(|_| StartupError::Invalid("Password hashing failed".into()))?;
        if count == 0 {
            let role: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE slug='admin'")
                .fetch_one(&mut *transaction)
                .await?;
            let token = match token.filter(|value| !value.is_empty()) {
                Some(value) => value,
                None => random_hex(32)?,
            };
            sqlx::query("INSERT INTO users(username,hash,api_key,role_id,password_change_required,last_login,account_locked) VALUES('admin',?,?,?,1,'0001-01-01 00:00:00',0)").bind(hash).bind(token).bind(role).execute(&mut *transaction).await?;
        } else if let Some(row) = existing.first() {
            let id: i64 = row.try_get("id")?;
            sqlx::query("UPDATE users SET hash=?,password_change_required=1 WHERE id=?")
                .bind(hash)
                .bind(id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query("INSERT INTO ruphish_auth_state(user_id,epoch) VALUES(?,1) ON CONFLICT(user_id) DO UPDATE SET epoch=epoch+1").bind(id).execute(&mut *transaction).await?;
        }
        transaction.commit().await?;
        Ok(Some(password))
    }

    /// Drain and close every database connection before process exit.
    pub async fn close(self) {
        self.pool.close().await;
    }
}

fn random_hex(length: usize) -> Result<String, StartupError> {
    let mut bytes = vec![0; length];
    getrandom::fill(&mut bytes)
        .map_err(|_| StartupError::Invalid("OS randomness unavailable".into()))?;
    Ok(hex::encode(bytes))
}

fn resolved(path: &Path) -> Result<PathBuf, std::io::Error> {
    if path.try_exists()? {
        return path.canonicalize();
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Database filename is missing",
        )
    })?;
    Ok(parent.canonicalize()?.join(name))
}

async fn startup_lock(path: &Path) -> Result<File, std::io::Error> {
    let mut name = path.as_os_str().to_owned();
    name.push(".ruphish-startup.lock");
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(PathBuf::from(name))?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(25)).await
            }
            Err(TryLockError::WouldBlock) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Another startup holds the database preparation lock",
                ));
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
    }
}

async fn prepare(
    connection: &mut SqliteConnection,
    path: &Path,
) -> Result<Option<PathBuf>, StartupError> {
    // Hold the write reservation before taking a snapshot: no committed writes can
    // appear between the recoverable backup and the migration transaction.
    let mut transaction = connection.begin_with("BEGIN IMMEDIATE").await?;
    let count = inspect(&mut transaction, false).await?;
    if count == MIGRATIONS.len() && super::auth_schema::validate_version(&mut transaction).await? {
        transaction.rollback().await?;
        return Ok(None);
    }
    let parent = path
        .parent()
        .ok_or_else(|| StartupError::Invalid("Database parent unavailable".into()))?;
    let temporary = tempfile::Builder::new()
        .prefix(".ruphish-backup-tmp-")
        .suffix(".sqlite3")
        .tempfile_in(parent)?;
    crate::private_file::protect(temporary.path())?;
    let mut reader = connect(path, true).await?;
    let snapshot = sqlx::query("VACUUM INTO ?")
        .bind(temporary.path().to_string_lossy().as_ref())
        .execute(&mut reader)
        .await;
    let closed = reader.close().await;
    snapshot?;
    closed?;
    let mut backup = connect(temporary.path(), true).await?;
    let verified = inspect(&mut backup, false).await;
    let closed = backup.close().await;
    if verified? != count {
        return Err(StartupError::Invalid(
            "Backup history changed during verification".into(),
        ));
    }
    closed?;
    temporary.as_file().sync_all()?;
    let backup_path = parent.join(format!("ruphish-pre-upgrade-{}.sqlite3", random_hex(16)?));
    let file = temporary
        .persist_noclobber(&backup_path)
        .map_err(|error| error.error)?;
    drop(file);
    apply_pending(&mut transaction, count).await?;
    super::auth_schema::migrate(&mut transaction).await?;
    inspect(&mut transaction, false).await?;
    transaction.commit().await?;
    Ok(Some(backup_path))
}

fn verify_migrations(prefix: &str) -> Result<(), StartupError> {
    let directory = PathBuf::from(format!("{prefix}sqlite3"));
    if matches!(prefix, "" | "db/db_" | "./db/db_") && !directory.try_exists()? {
        return Ok(());
    }
    let mut found = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        if entry.path().extension().is_none_or(|value| value != "sql") {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let version = name
            .split('_')
            .next()
            .and_then(|value| value.parse::<i64>().ok());
        let expected = MIGRATIONS.iter().find(|(id, _)| Some(*id) == version);
        let Some((id, source)) = expected else {
            return Err(StartupError::Invalid(
                "Custom migration directory contains an unsupported version".into(),
            ));
        };
        if !found.insert(*id)
            || std::fs::read_to_string(entry.path())?.replace("\r\n", "\n")
                != source.replace("\r\n", "\n")
        {
            return Err(StartupError::Invalid(
                "Custom migrations differ from the supported, verified migration set".into(),
            ));
        }
    }
    if found.len() != MIGRATIONS.len() {
        return Err(StartupError::Invalid(
            "Custom migration directory is incomplete".into(),
        ));
    }
    Ok(())
}
