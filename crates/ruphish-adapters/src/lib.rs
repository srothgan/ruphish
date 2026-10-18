//! Concrete persistence, delivery, import, template, and integration adapters.
//!
//! Adapters implement application interfaces without duplicating domain rules.

pub mod password_worker;
mod private_file;
pub mod sqlite;
pub mod template;
pub mod tls;
