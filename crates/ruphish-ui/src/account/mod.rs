//! Account modules.

#[cfg(target_arch = "wasm32")]
pub(crate) mod menu;
#[cfg(target_arch = "wasm32")]
pub(crate) mod pages;
#[cfg(target_arch = "wasm32")]
pub(crate) mod preferences;
pub mod preferences_state;
pub mod session;
#[cfg(target_arch = "wasm32")]
pub(crate) mod users;
