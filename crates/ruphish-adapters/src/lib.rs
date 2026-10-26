//! Concrete persistence, delivery, import, template, and integration adapters.
//!
//! Adapters implement application interfaces without duplicating domain rules.

pub mod database;
pub mod domain_verification;
mod email_html;
mod imap_limit;
pub mod imports;
pub mod landing;
mod mailbox_transport;
mod mysql;
pub mod outbound;
pub mod password_worker;
mod postgres;
pub mod private_file;
pub mod resources;
pub mod smtp;
mod sqlite;
pub mod template;
pub mod tls;
mod webhook_delivery;

mod campaign_preparation;
mod integration_work;
mod observation_schema;
mod participant_privacy;
mod previews;
mod public_interactions;
mod report_exports;
mod reporting;

mod campaign_listing;
