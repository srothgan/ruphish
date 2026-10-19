//! Browser-only admin application entry point.

#[cfg(not(target_arch = "wasm32"))]
compile_error!(
    "Build ruphish-ui for wasm32-unknown-unknown; native commands exclude this package."
);

#[cfg(target_arch = "wasm32")]
mod account;
#[cfg(target_arch = "wasm32")]
mod campaign_delivery;
#[cfg(target_arch = "wasm32")]
mod campaigns;
#[cfg(target_arch = "wasm32")]
mod components;
#[cfg(target_arch = "wasm32")]
mod domains;
#[cfg(target_arch = "wasm32")]
mod editor;
#[cfg(target_arch = "wasm32")]
mod feedback;
#[cfg(target_arch = "wasm32")]
mod navigation;
#[cfg(target_arch = "wasm32")]
mod resources;

#[cfg(target_arch = "wasm32")]
fn main() {
    use leptos::prelude::*;
    use leptos_router::components::Router;
    leptos::mount::mount_to_body(|| view! {<Router>{account::app}</Router>});
}
