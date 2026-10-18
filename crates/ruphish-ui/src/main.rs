//! Browser-only admin application entry point.

#[cfg(not(target_arch = "wasm32"))]
compile_error!(
    "Build ruphish-ui for wasm32-unknown-unknown; native commands exclude this package."
);

#[cfg(target_arch = "wasm32")]
mod editor;

#[cfg(target_arch = "wasm32")]
fn main() {
    leptos::mount::mount_to_body(editor::editor_workbench);
}
