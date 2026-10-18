# Local development and verification

Run commands from this product directory. rust-toolchain.toml selects Rust 1.95.0 with rustfmt, Clippy, and wasm32-unknown-unknown; edition 2024 and supported Rust 1.95 are inherited by every member. Rustup may download missing selected components. Trunk's exact requirement is in Trunk.toml, and cargo-deny's required version is in Cargo.toml workspace metadata. Add dependencies and install tools through CLI commands; keep Cargo.lock under version control.

## Direct checks

Run each command directly and report its result. AGENTS.md is the canonical command list. Do not introduce a script, Cargo alias layer, or npm task wrapper around these checks. Node is acceptable when needed for a development tool, but is not required by this workspace.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --exclude ruphish-ui --all-targets --locked -- -D warnings
cargo test --workspace --exclude ruphish-ui --locked
cargo build --workspace --exclude ruphish-ui --locked
cargo check -p ruphish-ui --target wasm32-unknown-unknown --locked
cargo clippy -p ruphish-ui --target wasm32-unknown-unknown --locked -- -D warnings
trunk build --release --locked
cargo deny check bans licenses sources advisories
```

No application feature flags exist yet, so these commands cover the current supported combinations. Introduce future SQLite/MySQL or alternative feature combinations with an explicit verification matrix; do not enable incompatible combinations with a blanket --all-features.

## Tests and checks

tests/workspace.rs is attached to the native executable package and runs through the native cargo test command. It checks actual Cargo metadata and manifests: package boundaries, local-source containment, inherited lint/package settings, supported compiler series, and deliberate native/browser default-member separation. Native libraries currently have no behavior tests because they contain no behavior. Inherited-case/functionality tests will be added with the features they verify; do not inflate counts with default Cargo arithmetic examples or tests mirroring trivial code.

Compatibility fixtures live in tests/fixtures/gophish-v0.12.1. The legacy map records planned destinations with not_implemented status. Copy database fixtures into isolated temporary locations before mutating them. No browser automation or screenshot suite is configured; the user owns visual and UI interaction testing.

Production code forbids unsafe code and denies accidental unwrap/expect/panic/exit/todo/unimplemented/dbg use. Clippy allows narrow assertion/setup patterns inside tests; ordinary helper functions still propagate errors rather than weakening the production lint policy. Every member inherits shared lints. rustfmt uses the pinned component's defaults.

## Browser assets

Trunk reads crates/ruphish-ui/index.html and its adjacent Cargo.toml, builds the wasm32-unknown-unknown binary using Cargo.lock, generates the matching wasm-bindgen JavaScript glue, and emits hashed files to dist/admin. Its wasm-bindgen tool is selected to match the locked crate, rather than maintained as a second manually synchronized version. Additional wasm-opt processing is explicitly disabled for this initial build; Rust release compilation and thin LTO still apply. Optimization/size decisions can be measured later.

Use `trunk serve` for a loopback-only manual UI preview. It does not start a Ruphish backend. There is no configured backend proxy or credential flow yet. The native scaffold likewise starts no listeners. Never use stale dist output as proof of current source behavior; rebuild after UI changes. target/, dist/, local data/, .env and PERSONAL.md are ignored. Regression SQLite fixtures are deliberately not globally ignored.

Trunk 0.21.14 parses NO_COLOR as a boolean in both root and subcommand options, so NO_COLOR=1 can fail before a build starts even with a --no-color flag. Set `$env:NO_COLOR = 'true'` in PowerShell before direct Trunk commands, or use `NO_COLOR=true trunk build --release --locked` in a POSIX shell. This is a process-only value; do not change persistent environment settings.

## Dependency policy

deny.toml evaluates Windows/Linux native and browser graphs, explicitly allows reviewed permissive licenses and crates.io, rejects unknown registry/Git sources, and checks advisories/yanked packages. Duplicate versions are reported for review rather than globally denied because third-party version splits do not necessarily imply an actionable conflict. Avoid broad suppressions. No advisory ignore entries are configured. Network access is required for initial dependency/tool downloads and current advisory information; a network failure is a failed/unavailable check, not a clean audit.

Two exact-version license exceptions cover existing Leptos transitive dependencies: base16 0.2.1 (its bundled LICENSE-CC0) and xxhash-rust 0.8.19 (its bundled Boost Software License). These do not permit those license identifiers globally or suppress security advisories. Dependency updates that leave those exact versions require policy review.

GitHub actions/state, releases, publishing, distribution packaging, and deployment are deferred. Completing local checks does not authorize any of those actions.
