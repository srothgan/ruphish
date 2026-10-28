//! Browser-only admin application entry point.
//!
//! The admin UI only runs in the browser, so every module and the real entry point are gated to
//! `wasm32`. Native commands exclude this package (`cargo ... --workspace --exclude ruphish-ui`),
//! but compiling every binary target is unavoidable for `cargo test -p ruphish-ui --lib --tests`,
//! so native builds produce an inert binary rather than failing to compile.

#[cfg(not(target_arch = "wasm32"))]
fn main() {}

#[cfg(target_arch = "wasm32")]
mod account;
#[cfg(target_arch = "wasm32")]
mod campaign_delivery;
#[cfg(target_arch = "wasm32")]
mod campaign_drafts;
#[cfg(target_arch = "wasm32")]
mod campaign_queries;
#[cfg(target_arch = "wasm32")]
mod campaign_results;
#[cfg(target_arch = "wasm32")]
mod campaigns;
#[cfg(target_arch = "wasm32")]
mod client;
#[cfg(target_arch = "wasm32")]
mod company_settings;
#[cfg(target_arch = "wasm32")]
mod domains;
#[cfg(target_arch = "wasm32")]
mod editor;
#[cfg(target_arch = "wasm32")]
mod feedback;
#[cfg(target_arch = "wasm32")]
mod integrations;
#[cfg(target_arch = "wasm32")]
mod management_queries;
#[cfg(target_arch = "wasm32")]
mod navigation;
#[cfg(target_arch = "wasm32")]
mod overview;
#[cfg(target_arch = "wasm32")]
mod overview_queries;
#[cfg(target_arch = "wasm32")]
mod preferences;
#[cfg(target_arch = "wasm32")]
mod resource_queries;
#[cfg(target_arch = "wasm32")]
mod resources;
#[cfg(target_arch = "wasm32")]
mod results_chart;
#[cfg(target_arch = "wasm32")]
mod shared;
#[cfg(target_arch = "wasm32")]
mod starter_templates;

#[cfg(target_arch = "wasm32")]
fn main() {
    use leptos::prelude::*;
    use leptos_router::components::Router;
    leptos::mount::mount_to_body(|| {
        use leptos_i18n::Locale as _;
        let context = ruphish_ui::localization::provide(ruphish_ui::localization::browser_locale());
        Effect::new(move |_| {
            let locale = context.get_locale();
            if let Some(document) = web_sys::window().and_then(|window| window.document())
                && let Some(html) = document.document_element()
            {
                let _ = html.set_attribute("lang", locale.as_str());
            }
        });
        shared::components::TablePreferences::provide();
        view! { <Router>{account::app}</Router> }
    });
}

#[cfg(target_arch = "wasm32")]
mod time_display;
