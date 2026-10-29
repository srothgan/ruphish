//! Authentication and account use cases, independent of HTTP and SQL.
pub use ruphish_domain::account::{Account, AccountError, Edit, Identity, Role};
pub use ruphish_domain::password::PasswordError;
pub use ruphish_domain::preferences::Preferences;
use ruphish_domain::{account, password};
use std::sync::Arc;

/// An application failure with safe public messages.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Independent field failures, separate from authentication and transport errors.
    #[error(transparent)]
    Validation(#[from] ruphish_domain::validation::Report),
    /// Safe persistence classification retained for reconciliation and retry policy.
    #[error("Account operation could not be completed")]
    Storage(#[from] crate::storage::Error),
    /// Domain authorization or account rule.
    #[error(transparent)]
    Policy(#[from] AccountError),
    /// Password validation rule.
    #[error(transparent)]
    Password(#[from] password::PasswordError),
    /// Credentials do not verify.
    #[error("Invalid Username/Password")]
    InvalidCredentials,
    /// Requested account is absent.
    #[error("User not found")]
    NotFound,
    /// A unique account name is already assigned.
    #[error("Username already taken")]
    Conflict,
    /// Unsupported role.
    #[error("Role not found")]
    RoleNotFound,
    /// A concurrent credential or account update requires retrying with fresh state.
    #[error("Account changed; sign in again")]
    Changed,
    /// Authentication resources are temporarily exhausted.
    #[error("Authentication is busy; try again shortly")]
    Busy,
    /// Input exceeds the supported request/credential limit.
    #[error("Password exceeds the supported length")]
    TooLong,
    /// Persistence or cryptographic failure, with details restricted to safe diagnostics.
    #[error("Account operation could not be completed")]
    Internal,
}

/// Prepared write executed atomically with current authorization checks.
pub enum Write {
    /// Replace the caller's display preferences without revoking sessions.
    Preferences(Preferences),
    /// Create or update a user, preserving unrelated fields.
    Save {
        /// Existing account identity for optimistic concurrency, or a new user.
        target: Option<Identity>,
        /// Validated input.
        edit: Edit,
        /// Prepared hash, computed without holding a database transaction.
        hash: Option<String>,
    },
    /// Delete an account and its owned legacy records.
    Delete(i64),
    /// Change the caller's password and clear the required-reset flag.
    Password {
        /// Original encoded hash for compare-and-swap.
        expected_hash: String,
        /// New encoded hash.
        hash: String,
    },
    /// Replace the caller's API credential.
    RotateKey,
    /// Record a successful login without overwriting concurrent account edits.
    Login,
}

/// Real account storage; writes recheck identity and policy inside a transaction.
#[async_trait::async_trait]
pub trait Repository: Send + Sync {
    /// Lookup by immutable identifier.
    async fn by_id(&self, id: i64) -> Result<Account, Error>;
    /// Lookup by exact legacy username.
    async fn by_name(&self, name: &str) -> Result<Account, Error>;
    /// Resolve an existing API credential.
    async fn by_key(&self, key: &str) -> Result<Account, Error>;
    /// Enumerate accounts after rechecking administrative permission.
    async fn list(&self, caller: Identity) -> Result<Vec<Account>, Error>;
    /// Commit a guarded write and return the resulting account if it still exists.
    async fn write(&self, caller: Identity, write: Write) -> Result<Option<Account>, Error>;
}

/// Bounded cryptographic work implemented by established password libraries.
#[async_trait::async_trait]
pub trait Passwords: Send + Sync {
    /// Verify a stored bcrypt or Argon2id hash.
    async fn verify(&self, password: String, hash: String) -> Result<bool, Error>;
    /// Generate a new Argon2id hash.
    async fn hash(&self, password: String) -> Result<String, Error>;
    /// Perform comparable work for an unknown username.
    async fn dummy(&self, password: String) -> Result<(), Error>;
}

/// Application entry point for transport-independent account workflows.
#[derive(Clone)]
pub struct Service {
    repository: Arc<dyn Repository>,
    passwords: Arc<dyn Passwords>,
}

impl Service {
    /// Save only the authenticated caller's display overrides.
    pub async fn save_preferences(
        &self,
        caller: Identity,
        preferences: Preferences,
    ) -> Result<Preferences, Error> {
        Ok(self
            .repository
            .write(caller, Write::Preferences(preferences))
            .await?
            .ok_or(Error::Internal)?
            .preferences)
    }
    /// Compose the concrete repository and password worker.
    pub fn new(repository: Arc<dyn Repository>, passwords: Arc<dyn Passwords>) -> Self {
        Self {
            repository,
            passwords,
        }
    }
    /// Authenticate and record login using the current account generation.
    pub async fn login(&self, username: &str, password: String) -> Result<Account, Error> {
        if password.len() > password::MAX_BYTES {
            return Err(Error::InvalidCredentials);
        }
        let user = match self.repository.by_name(username).await {
            Ok(user) => user,
            Err(Error::NotFound) => {
                self.passwords.dummy(password).await?;
                return Err(Error::InvalidCredentials);
            }
            Err(error) => return Err(error),
        };
        if !self.passwords.verify(password, user.hash.clone()).await? || user.account_locked {
            return Err(Error::InvalidCredentials);
        }
        self.repository
            .write(user.identity, Write::Login)
            .await?
            .ok_or(Error::Internal)
    }
    /// Validate a session against live persistent account state.
    pub async fn session(&self, identity: Identity) -> Result<Account, Error> {
        let user = self.repository.by_id(identity.id).await?;
        user.authenticate(identity)?;
        Ok(user)
    }
    /// Resolve a machine credential and reject locked or incomplete accounts.
    pub async fn api_key(&self, key: &str) -> Result<Account, Error> {
        let user = self.repository.by_key(key).await?;
        user.authenticate(user.identity)?;
        if user.password_change_required {
            return Err(AccountError::ResetRequired.into());
        }
        Ok(user)
    }
    /// Load an authorized target account.
    pub async fn account(&self, caller: Identity, id: i64) -> Result<Account, Error> {
        self.session(caller).await?.access_account(id, false)?;
        self.repository.by_id(id).await
    }
    /// Enumerate users for an administrator.
    pub async fn list(&self, caller: Identity) -> Result<Vec<Account>, Error> {
        self.repository.list(caller).await
    }
    /// Validate and prepare an account edit before its guarded transaction.
    pub async fn save(
        &self,
        caller: Identity,
        id: Option<i64>,
        edit: Edit,
    ) -> Result<Account, Error> {
        let actor = self.session(caller).await?;
        // Credential replacement for oneself belongs to change_password, which
        // verifies the current credential (or the mandatory setup state).
        // Keep administrator resets of other accounts as a distinct permission.
        if id == Some(actor.identity.id) && !edit.password.is_empty() {
            return Err(Error::InvalidCredentials);
        }
        let target = if let Some(id) = id {
            actor.access_account(id, true)?;
            Some(self.repository.by_id(id).await?.identity)
        } else {
            actor.require("modify_system")?;
            None
        };
        let mut report = account::validation(&edit.username, &edit.role);
        if target.is_none() || !edit.password.is_empty() {
            report.extend(password::validation(&edit.password, "password"));
        }
        report.finish()?;
        let hash = if target.is_none() || !edit.password.is_empty() {
            Some(self.passwords.hash(edit.password.clone()).await?)
        } else {
            None
        };
        self.repository
            .write(caller, Write::Save { target, edit, hash })
            .await?
            .ok_or(Error::Internal)
    }
    /// Delete with transactional ownership cleanup and administrator protection.
    pub async fn delete(&self, caller: Identity, id: i64) -> Result<(), Error> {
        self.repository.write(caller, Write::Delete(id)).await?;
        Ok(())
    }
    /// Rotate an API key and invalidate its predecessor atomically.
    pub async fn rotate_key(&self, caller: Identity) -> Result<Account, Error> {
        self.repository
            .write(caller, Write::RotateKey)
            .await?
            .ok_or(Error::Internal)
    }
    /// Change a password after current-password authentication or mandatory setup.
    pub async fn change_password(
        &self,
        caller: Identity,
        current: Option<String>,
        candidate: String,
        confirmation: &str,
    ) -> Result<Account, Error> {
        let user = self.session(caller).await?;
        if (!user.password_change_required || current.is_some())
            && !self
                .passwords
                .verify(current.unwrap_or_default(), user.hash.clone())
                .await?
        {
            return Err(Error::InvalidCredentials);
        }
        password::replacement(&candidate, confirmation).finish()?;
        if self
            .passwords
            .verify(candidate.clone(), user.hash.clone())
            .await?
        {
            return Err(password::PasswordError::Reused.into());
        }
        let hash = self.passwords.hash(candidate).await?;
        self.repository
            .write(
                caller,
                Write::Password {
                    expected_hash: user.hash,
                    hash,
                },
            )
            .await?
            .ok_or(Error::Internal)
    }
    /// Authorize the administrator and target for impersonation.
    pub async fn impersonate(&self, caller: Identity, username: &str) -> Result<Account, Error> {
        self.session(caller).await?.require("modify_system")?;
        let target = self.repository.by_name(username).await?;
        target.authenticate(target.identity)?;
        if target.password_change_required {
            return Err(AccountError::ResetRequired.into());
        }
        Ok(target)
    }
}
