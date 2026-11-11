//! Results modules.

#[cfg(target_arch = "wasm32")]
pub(crate) mod chart;
#[cfg(target_arch = "wasm32")]
pub(crate) mod page;
#[cfg(target_arch = "wasm32")]
pub(crate) mod recipient;
pub mod state;
