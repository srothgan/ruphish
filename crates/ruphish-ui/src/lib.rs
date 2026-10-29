//! Editing state shared by the browser component and native workflow tests.

#[cfg(target_arch = "wasm32")]
pub mod localization;

pub mod campaign_state;
pub mod deletion_state;
#[cfg(target_arch = "wasm32")]
pub mod import_drafts;
pub mod import_state;
pub mod preference_state;
pub mod preview_state;
pub mod read_state;
pub mod recipient_history;
pub mod resource_inventory;
pub mod resource_state;
pub mod results_state;
pub mod select_options;
pub mod session;
pub mod table_state;

#[cfg(target_arch = "wasm32")]
pub mod browser_http;

#[cfg(target_arch = "wasm32")]
pub mod view_tasks;

#[cfg(target_arch = "wasm32")]
pub mod query;

pub mod api_error;
pub mod form_state;
pub mod form_validation;
pub mod settings_state;

/// Build an isolated preview document without changing the stored source.
///
/// The browser also applies an empty iframe sandbox, inert interaction, and no referrer.
/// Network resources, scripts, frame loads and form submission are blocked in preview.
pub fn preview_document(source: &str) -> String {
    format!(
        "<!doctype html><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src data: blob:; form-action 'none'; base-uri 'none';\">{source}"
    )
}

/// Functional landing preview; pair only with the domain's opaque-origin SANDBOX.
/// The server supplies managed submission feedback with no live event endpoint.
pub fn landing_preview_document(source: &str) -> String {
    format!(
        "<!doctype html><meta http-equiv=\"Content-Security-Policy\" content=\"{}\">{source}",
        ruphish_domain::landing::CONTENT_POLICY
    )
}

#[cfg(test)]
#[path = "../../../tests/editor_workflow.rs"]
mod editor_workflow;

pub mod tabs;

pub mod overview_state;

pub mod datetime;

#[cfg(target_arch = "wasm32")]
pub mod browser_datetime;

#[cfg(target_arch = "wasm32")]
pub mod display_preferences;

#[cfg(target_arch = "wasm32")]
pub mod local_storage;
pub mod starter_names;
pub mod starter_selection;

pub mod event_editor;
