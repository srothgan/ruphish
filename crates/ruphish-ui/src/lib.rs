//! Editing state shared by the browser component and native workflow tests.

#[cfg(target_arch = "wasm32")]
pub mod import_drafts;
pub mod import_state;
pub mod resource_state;
pub mod session;

#[cfg(target_arch = "wasm32")]
pub mod browser_http;

/// Exported Go template fields offered by the editor.
pub const VARIABLES: &[&str] = &[
    "FirstName",
    "LastName",
    "Email",
    "Position",
    "From",
    "URL",
    "RId",
    "BaseURL",
    "TrackingURL",
    "Tracker",
];

/// Build an isolated preview document without changing the stored source.
///
/// The browser also applies an empty iframe sandbox, inert interaction, and no referrer.
/// Network resources, scripts, frame loads and form submission are blocked in preview.
pub fn preview_document(source: &str) -> String {
    format!(
        "<!doctype html><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src data: blob:; form-action 'none'; base-uri 'none';\">{source}"
    )
}

#[cfg(test)]
#[path = "../../../tests/editor_workflow.rs"]
mod editor_workflow;
