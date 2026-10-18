//! Native executable composition root.
//!
//! The workspace entry point currently prints its version. Configuration,
//! listeners, integrations, and background workers are implemented in later work.

fn main() {
    println!("Ruphish {}", env!("CARGO_PKG_VERSION"));
}
