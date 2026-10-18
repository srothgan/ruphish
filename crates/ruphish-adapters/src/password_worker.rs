//! Bounded password work using bcrypt for existing hashes and Argon2id for new ones.
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use ruphish_application::auth::{Error, Passwords};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Shared process-wide admission limit for CPU/memory-intensive password operations.
#[derive(Clone)]
pub struct PasswordWorker {
    capacity: Arc<Semaphore>,
}
impl Default for PasswordWorker {
    fn default() -> Self {
        Self {
            capacity: Arc::new(Semaphore::new(4)),
        }
    }
}
impl PasswordWorker {
    async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        let permit = self
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Busy)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        })
        .await
        .map_err(|_| Error::Internal)?
    }
}
/// Verify existing credential formats without changing stored material.
pub fn verify(password: &str, hash: &str) -> bool {
    if password.len() > 1024 {
        return false;
    }
    if hash.starts_with("$argon2id$") {
        PasswordHash::new(hash).is_ok_and(|hash| {
            // Stored parameters are untrusted too; bound memory and CPU before allocation.
            argon2::Params::try_from(&hash).is_ok_and(|params| {
                params.m_cost() <= 65_536 && params.t_cost() <= 10 && params.p_cost() <= 4
            }) && Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .is_ok()
        })
    } else if hash.starts_with("$2") {
        // Preserve verification of old Go hashes, including its historical byte truncation.
        // Newly set passwords use Argon2id and authenticate the complete input instead.
        hash.split('$')
            .nth(2)
            .and_then(|cost| cost.parse::<u32>().ok())
            .is_some_and(|cost| cost <= 16)
            && bcrypt::verify(password.as_bytes(), hash).unwrap_or(false)
    } else {
        false
    }
}
/// Produce the configured modern format. Called only from bounded blocking workers.
pub fn hash(password: &str) -> Result<String, Error> {
    if password.len() > 1024 {
        return Err(Error::TooLong);
    }
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|_| Error::Internal)
}
#[async_trait::async_trait]
impl Passwords for PasswordWorker {
    async fn verify(&self, password: String, encoded: String) -> Result<bool, Error> {
        self.run(move || Ok(verify(&password, &encoded))).await
    }
    async fn hash(&self, password: String) -> Result<String, Error> {
        self.run(move || hash(&password)).await
    }
    async fn dummy(&self, password: String) -> Result<(), Error> {
        self.run(move || hash(&password).map(|_| ())).await
    }
}
