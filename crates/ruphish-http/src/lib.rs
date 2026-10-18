//! Administrative and public HTTP boundaries, middleware, and contract mapping.
//!
//! Handlers coordinate application operations; executable composition supplies
//! concrete adapters. No application routes have been implemented yet.

pub mod auth;
mod rate;
pub mod response;
pub mod server;
