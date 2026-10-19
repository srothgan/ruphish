//! Application use cases, transaction boundaries, and external-service interfaces.
//!
//! Use cases coordinate the domain without concrete HTTP handlers, database
//! drivers, delivery clients, or the admin UI.
pub mod auth;
pub mod campaign;
pub mod delivery;
pub mod domains;
pub mod interactions;
pub mod resources;
