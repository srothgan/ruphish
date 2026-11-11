//! Authoritative domain types, invariants, permissions, and state transitions.
//!
//! This boundary has no HTTP, persistence, runtime, or browser dependencies.

pub mod account;
pub mod attribution;
pub mod campaign;
pub mod datetime;
pub mod domains;
pub mod events;
pub mod integrations;
pub mod landing;
pub mod mail_credentials;
pub mod navigation;
pub mod pagination;
pub mod profile_diagnostics;
pub mod reporting;
pub mod resources;
pub mod settings;
pub mod template;
pub mod validation;
