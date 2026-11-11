//! Concrete persistence, delivery, import, template, and integration adapters.
//!
//! Adapters implement application interfaces without duplicating domain rules.

pub(crate) mod campaigns;
pub mod database;
pub mod domain_verification;
mod email_html;
mod event_edit;
pub mod imports;
pub(crate) mod integrations;
pub mod landing;
mod observation_schema;
pub mod outbound;
mod participant_privacy;
pub mod password_worker;
mod previews;
pub mod private_file;
pub(crate) mod profile_diagnostics;
mod public_interactions;
mod reporting;
pub mod resources;
pub mod rotating_log;
pub mod smtp;
mod sql;
pub mod template;
pub mod tls;
