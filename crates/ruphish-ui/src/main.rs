//! Browser entry point; feature modules and state belong to the UI library.

#[cfg(not(target_arch = "wasm32"))]
fn main() {}

#[cfg(target_arch = "wasm32")]
fn main() {
    ruphish_ui::mount();
}
