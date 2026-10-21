//! Browser-safe wire representations for the preserved administrative HTTP API.
//!
//! Wire types belong here; backend credentials, database records, and domain
//! validation authority do not. Contracts must compile for native Rust and WASM.

pub mod auth;
pub mod campaign;
pub mod campaign_reads;
pub mod campaign_workflow;
pub mod domains;
pub mod legacy;
pub mod reporting;

pub mod overview;

pub mod errors;
pub mod integrations;
pub mod resource_details;
pub mod resources;

// An explicit null is meaningful; an absent property is an incomplete response.
fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(deserializer)
}
