//! Authoritative domain types, invariants, permissions, and state transitions.
//!
//! This boundary has no HTTP, persistence, runtime, or browser dependencies.
pub mod account;
pub mod campaign;
pub mod domains;
pub mod password;
pub mod reporting;
pub mod resources;

pub mod datetime;
