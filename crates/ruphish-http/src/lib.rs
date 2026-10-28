//! Administrative and public HTTP boundaries, middleware, and contract mapping.
//!
//! Handlers coordinate application operations; executable composition supplies
//! concrete adapters.

pub mod auth;
mod campaigns;
mod domains;
pub mod health;
mod integrations;
pub mod interactions;
mod overview;
pub mod public_assets;
mod rate;
mod reporting;
mod resources;
pub mod response;
pub mod server;
mod starter_templates;

mod settings;

mod event_edit;
