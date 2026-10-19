//! Administrative and public HTTP boundaries, middleware, and contract mapping.
//!
//! Handlers coordinate application operations; executable composition supplies
//! concrete adapters.

pub mod auth;
mod campaigns;
mod domains;
pub mod interactions;
pub mod public_assets;
mod rate;
mod resources;
pub mod response;
pub mod server;
