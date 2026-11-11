//! Api modules.

#[cfg(target_arch = "wasm32")]
pub(crate) mod client;
pub mod error;
#[cfg(target_arch = "wasm32")]
pub mod query;
pub mod read_state;
#[cfg(target_arch = "wasm32")]
pub mod transport;
