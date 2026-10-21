//! Browser-only admin application entry point.

#[cfg(not(target_arch = "wasm32"))]
compile_error!(
    "Build ruphish-ui for wasm32-unknown-unknown; native commands exclude this package."
);

#[cfg(target_arch = "wasm32")]
mod account;
#[cfg(target_arch = "wasm32")]
mod analysis;
#[cfg(target_arch = "wasm32")]
mod analysis_chart;
#[cfg(target_arch = "wasm32")]
mod campaign_delivery;
#[cfg(target_arch = "wasm32")]
mod campaigns;
#[cfg(target_arch = "wasm32")]
mod domains;
#[cfg(target_arch = "wasm32")]
mod editor;
#[cfg(target_arch = "wasm32")]
mod feedback;
#[cfg(target_arch = "wasm32")]
mod integrations;
#[cfg(target_arch = "wasm32")]
mod navigation;
#[cfg(target_arch = "wasm32")]
mod overview;
#[cfg(target_arch = "wasm32")]
mod preferences;
#[cfg(target_arch = "wasm32")]
mod resources;
#[cfg(target_arch = "wasm32")]
mod shared;

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
        view! {<Router>{account::app}</Router>}
    });
}

#[cfg(target_arch = "wasm32")]
mod time_display;
