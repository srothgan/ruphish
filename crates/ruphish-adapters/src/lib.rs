//! Concrete persistence, delivery, import, template, and integration adapters.
//!
//! Adapters implement application interfaces without duplicating domain rules.

pub mod domain_verification;
mod imap_limit;
pub mod imports;
pub mod landing;
mod mailbox_transport;
pub mod outbound;
pub mod password_worker;
mod private_file;
pub mod resources;
pub mod smtp;
pub mod sqlite;
pub mod template;
pub mod tls;
mod webhook_delivery;
