//! Account authorization rules shared by every transport.

/// The authenticated identity and its revocation generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    /// Persistent user identifier.
    pub id: i64,
    /// Incremented when credentials or account privileges change.
    pub epoch: i64,
}

/// Role metadata and effective permissions loaded from persistent storage.
#[derive(Clone, Debug)]
pub struct Role {
    /// Persistent role identifier.
    pub id: i64,
    /// Legacy role slug.
    pub slug: String,
    /// Display name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Permissions, including custom persisted role mappings.
    pub permissions: Vec<String>,
}

/// A backend account; deliberately not serializable or printable.
#[derive(Clone)]
pub struct Account {
    /// User identifier and revocation generation.
    pub identity: Identity,
    /// Login name, preserved without normalization.
    pub username: String,
    /// Encoded password hash, never sent to clients.
    pub hash: String,
    /// Legacy API credential, only disclosed by authorized wire contracts.
    pub api_key: String,
    /// Current role and permissions.
    pub role: Role,
    /// Access is limited to completing password setup.
    pub password_change_required: bool,
    /// Every authentication path must reject a locked account.
    pub account_locked: bool,
    /// Legacy last-login timestamp.
    pub last_login: String,
}

/// Observable account-policy rejection.
#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    /// Caller no longer has a valid authenticated identity.
    #[error("Authentication required")]
    Unauthorized,
    /// The operation requires stronger authorization.
    #[error("Forbidden")]
    Forbidden,
    /// Password setup is incomplete.
    #[error("Password change required")]
    ResetRequired,
    /// Usernames must not be empty.
    #[error("No username provided")]
    EmptyUsername,
    /// A role must be supplied.
    #[error("No role specified")]
    EmptyRole,
    /// An ordinary user cannot edit privileged account attributes.
    #[error("Permission denied")]
    Escalation,
    /// Administrative access must remain possible.
    #[error("Cannot remove the only administrator")]
    LastAdmin,
    /// Account deletion must be performed by another authorized account.
    #[error("You cannot delete your own account")]
    SelfDeletion,
}

impl Account {
    /// Authorize deletion without allowing the caller to remove its own identity.
    pub fn delete_account(&self, target: i64) -> Result<(), AccountError> {
        self.access_account(target, true)?;
        if self.identity.id == target {
            return Err(AccountError::SelfDeletion);
        }
        Ok(())
    }
    /// Reject revoked, missing or locked identities before authorization.
    pub fn authenticate(&self, identity: Identity) -> Result<(), AccountError> {
        if self.account_locked || self.identity != identity {
            return Err(AccountError::Unauthorized);
        }
        Ok(())
    }

    /// Determine a persisted permission without guessing from a role name.
    pub fn permits(&self, permission: &str) -> bool {
        self.role.permissions.iter().any(|p| p == permission)
    }

    /// Require completed password setup and the requested permission.
    pub fn require(&self, permission: &str) -> Result<(), AccountError> {
        if self.password_change_required {
            return Err(AccountError::ResetRequired);
        }
        if !self.permits(permission) {
            return Err(AccountError::Forbidden);
        }
        Ok(())
    }

    /// Authorize access to an account, independently of the request's method.
    pub fn access_account(&self, target: i64, write: bool) -> Result<(), AccountError> {
        if self.password_change_required {
            return Err(AccountError::ResetRequired);
        }
        if write {
            self.require("modify_objects")?;
        }
        if self.identity.id != target && !self.permits("modify_system") {
            return Err(AccountError::Forbidden);
        }
        Ok(())
    }
}

/// Validate identity fields without changing the user's spelling or whitespace.
pub fn validate_fields(username: &str, role: &str) -> Result<(), AccountError> {
    if username.is_empty() {
        return Err(AccountError::EmptyUsername);
    }
    if role.is_empty() {
        return Err(AccountError::EmptyRole);
    }
    Ok(())
}

/// Reject deleting, locking or demoting the last unlocked administrator.
pub fn preserve_administrator(
    was_available: bool,
    remains_available: bool,
    available: i64,
) -> Result<(), AccountError> {
    if was_available && !remains_available && available <= 1 {
        return Err(AccountError::LastAdmin);
    }
    Ok(())
}
