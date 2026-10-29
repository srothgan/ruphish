//! Application use cases, transaction boundaries, and external-service interfaces.
//!
//! Use cases coordinate the domain without concrete HTTP handlers, database
//! drivers, delivery clients, or the admin UI.
pub mod auth;
pub mod campaign;
pub mod content_work;
pub mod delivery;
pub mod domains;
pub mod health;
pub mod integrations;
pub mod interactions;
pub mod overview;
pub mod reporting;
pub mod resources;
pub mod settings;
pub mod storage;

/// Shared date/time rules used by application consumers.
pub use ruphish_domain::datetime;

/// Pure validation results exposed to transport adapters.
pub use ruphish_domain::validation;

/// Shared sorted-continuation rules used by transport and storage adapters.
pub use ruphish_domain::pagination;

pub mod event_edit;
