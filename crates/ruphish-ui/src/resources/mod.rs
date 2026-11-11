//! Resources modules.

#[cfg(target_arch = "wasm32")]
pub(crate) mod content_editor;
pub mod draft;
pub mod imports;
pub mod inventory;
#[cfg(target_arch = "wasm32")]
pub(crate) mod pages;
pub mod preview_state;
#[cfg(target_arch = "wasm32")]
pub(crate) mod queries;
pub mod recipients;
#[cfg(target_arch = "wasm32")]
pub(crate) mod view;
