//! Shared admin presentation.

#[cfg(target_arch = "wasm32")]
pub(crate) mod components;
pub mod datetime;
pub mod deletion;
#[cfg(target_arch = "wasm32")]
pub mod display_preferences;
#[cfg(target_arch = "wasm32")]
pub(crate) mod feedback;
pub mod forms;
#[cfg(target_arch = "wasm32")]
pub mod local_storage;
#[cfg(target_arch = "wasm32")]
pub(crate) mod navigation;
pub mod select_options;
pub mod tab_navigation;
pub mod table_sort;
#[cfg(target_arch = "wasm32")]
pub mod tasks;
