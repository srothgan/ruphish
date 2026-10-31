//! Authoritative domain types, invariants, permissions, and state transitions.
//!
//! This boundary has no HTTP, persistence, runtime, or browser dependencies.
pub mod account;
pub mod campaign;
pub mod domains;
pub mod pagination;
pub mod password;
pub mod preferences;
pub mod reporting;
pub mod resources;

pub mod datetime;

pub mod campaign_draft;

pub mod integrations;

pub mod template;

pub mod events;
pub mod settings;
pub mod validation;

pub mod event_edit;
pub mod landing;

pub mod client;
