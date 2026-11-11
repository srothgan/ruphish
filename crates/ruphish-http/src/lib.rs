//! Administrative and public HTTP boundaries, middleware, and contract mapping.
//!
//! Handlers coordinate application operations; executable composition supplies
//! concrete adapters.

pub mod assets;
mod attribution;
pub mod auth;
mod campaigns;
mod domains;
mod event_edit;
pub mod health;
mod integrations;
pub mod interactions;
mod overview;
mod reporting;
mod resources;
pub mod response;
mod revision;
pub mod server;
mod settings;
mod starter_templates;

#[path = "../asset_distribution.rs"]
mod asset_distribution;
