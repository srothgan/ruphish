//! Browser-safe wire representations for the preserved administrative HTTP API.
//!
//! Wire types belong here; backend credentials, database records, and domain
//! validation authority do not. Contracts must compile for native Rust and WASM.

pub mod auth;
pub mod campaign;
pub mod domains;
pub mod legacy;
pub mod reporting;

pub mod overview;
