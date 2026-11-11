//! Administration features shared by the browser and native workflow tests.

pub mod account;
pub mod api;
pub mod campaigns;
#[cfg(target_arch = "wasm32")]
pub(crate) mod domains;
#[cfg(target_arch = "wasm32")]
pub(crate) mod integrations;
#[cfg(target_arch = "wasm32")]
pub mod localization;
#[cfg(target_arch = "wasm32")]
pub(crate) mod management_queries;
pub mod overview;
#[cfg(target_arch = "wasm32")]
pub(crate) mod profile_diagnostics;
pub mod resources;
pub mod settings;
pub mod shared;
pub mod starter_templates;

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

/// Mount the browser administration interface.
#[cfg(target_arch = "wasm32")]
pub fn mount() {
    use leptos::prelude::*;
    use leptos_router::components::Router;
    leptos::mount::mount_to_body(|| {
        use leptos_i18n::Locale as _;
        let context = crate::localization::provide(crate::localization::browser_locale());
        Effect::new(move |_| {
            let locale = context.get_locale();
            if let Some(document) = web_sys::window().and_then(|window| window.document())
                && let Some(html) = document.document_element()
            {
                let _ = html.set_attribute("lang", locale.as_str());
            }
        });
        shared::components::TablePreferences::provide();
        view! { <Router>{account::pages::app}</Router> }
    });
}
