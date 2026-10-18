# Ruphish

Ruphish is a Rust backend and Rust/WASM admin application being built to preserve Gophish's supported external contracts while modernizing its implementation and interface.

The native application now loads legacy configuration, prepares SQLite with recoverable upgrades, bootstraps the administrator, serves built admin assets and public listeners over HTTP/TLS, and shuts down on native signals. Compatibility proofs and the local CodeMirror/Jodit template workbench remain covered by regression tests. Login, resource APIs, campaign execution and API-connected UI remain later work. The user reports the workbench is functional; UI/UX polish and detailed browser/security checks remain open.

## Getting started

Rustup reads the exact compiler/components/browser target from rust-toolchain.toml. Install dependencies/tools through CLI commands rather than writing dependency versions into manifests:

```sh
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy --target wasm32-unknown-unknown
cargo install trunk --version 0.21.14 --locked
cargo install cargo-deny --version 0.19.9 --locked
cargo fetch --locked
npm ci --prefix crates/ruphish-ui/web
trunk build --release --locked
cargo build --workspace --exclude ruphish-ui --locked
cargo test --workspace --exclude ruphish-ui --locked
cargo run -p ruphish --locked -- --help
```

Windows native compilation requires the Visual Studio C++ build tools/Windows SDK; other hosts require their normal Rust native linker. Dependencies are resolved in Cargo.lock. The declared supported compiler is Rust 1.99; older versions have not been verified. Node and npm are required to install, test and bundle the CodeMirror browser component. Go and Python are only needed for the separate rewrite evidence tools.

Run the direct commands listed in [AGENTS.md](AGENTS.md) for formatting, Clippy, tests, native/WASM builds, and dependency checks. No PowerShell/Node task runner or Cargo alias layer is required. Copy config.example.json to config.json before starting listeners; see [startup and database recovery](docs/startup.md). See [development](docs/development.md) for details and [architecture](docs/architecture.md) for boundaries. If your environment exports NO_COLOR=1, set its process value to true before invoking Trunk, as documented there.

## Source and generated files

Seven crates live in crates/. Regression data and legacy-case obligations live in tests/. Product tooling and documentation live in tools/ and docs/. Build output in target/ and browser output in dist/admin are generated and ignored; rebuild them from source.

The compatibility corpus contains synthetic data derived from Gophish Community Edition 0.12.1. Its upstream MIT notice is preserved in LICENSE. Never use fixture credentials for an actual installation. SQLite is the first feature-complete target, with MySQL planned afterward. Publishing, releases, distribution packaging, and deployment are outside the current implementation scope.
