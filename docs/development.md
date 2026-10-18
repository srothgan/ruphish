# Local development and verification

Run commands from this product directory. rust-toolchain.toml selects Rust 1.99.1 with rustfmt, Clippy, and wasm32-unknown-unknown; edition 2024 and supported Rust 1.99 are inherited by every member. Rustup may download missing selected components. Trunk's exact requirement is in Trunk.toml, and cargo-deny's required version is in Cargo.toml workspace metadata. Add dependencies and install tools through CLI commands; keep Cargo.lock under version control.

## Direct checks

Run each command directly and report its result. AGENTS.md is the canonical command list. Do not introduce a script, Cargo alias layer, or npm task wrapper around these checks. Node and npm install, test and bundle the CodeMirror browser component; Node is not a server runtime dependency.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --exclude ruphish-ui --all-targets --locked -- -D warnings
cargo test --workspace --exclude ruphish-ui --locked
cargo build --workspace --exclude ruphish-ui --locked
npm ci --prefix crates/ruphish-ui/web
node --check crates/ruphish-ui/web/build.mjs
node --check crates/ruphish-ui/web/state.mjs
node --check crates/ruphish-ui/web/editor.mjs
node --check crates/ruphish-ui/web/template-html.mjs
node --check crates/ruphish-ui/web/visual-session.mjs
node --check crates/ruphish-ui/web/visual-frame.mjs
node --check crates/ruphish-ui/web/visual-shell.mjs
node --check crates/ruphish-ui/web/visual-controller.mjs
node --test crates/ruphish-ui/web/tests/*.test.mjs
cargo test -p ruphish-ui --lib --locked
cargo clippy -p ruphish-ui --lib --locked -- -D warnings
node crates/ruphish-ui/web/build.mjs
cargo check -p ruphish-ui --target wasm32-unknown-unknown --locked
cargo clippy -p ruphish-ui --target wasm32-unknown-unknown --locked -- -D warnings
trunk build --release --locked
npm audit --prefix crates/ruphish-ui/web
cargo deny check bans licenses sources advisories
```

No application feature flags exist yet, so these commands cover the current supported combinations. Introduce future SQLite/MySQL or alternative feature combinations with an explicit verification matrix; do not enable incompatible combinations with a blanket --all-features.

## Tests and checks

tests/workspace.rs is attached to the native executable package and runs through the native cargo test command. It checks actual Cargo metadata and manifests: package boundaries, local-source containment, inherited lint/package settings, supported compiler series, and deliberate native/browser default-member separation. Compatibility tests now exercise real SQLite upgrades, stored passwords/templates, and loopback HTTP wire behavior. UI library workflows cover template import/export, cancellation and binary attachments; separate Node tests use actual CodeMirror state/history/completion APIs with the browser bridge's configuration. These are focused proofs, not full resource/authentication/campaign coverage.

Original compatibility fixtures live in tests/fixtures/gophish-v0.12.1; additional Go-captured upgrade/wire/editor fixtures live in tests/fixtures/compatibility. Authentication fixtures in tests/fixtures/authentication preserve independent Go account API responses and an established bcrypt account database. The legacy map identifies implemented equivalents and their exact executable tests; remaining feature cases retain not_implemented status. Copy database fixtures into isolated temporary locations before mutating them. No browser automation or screenshot suite is configured; the user owns visual and UI interaction testing.

Production code forbids unsafe code and denies accidental unwrap/expect/panic/exit/todo/unimplemented/dbg use. Clippy allows narrow assertion/setup patterns inside tests; the isolated executable-test harness has local unwrap/panic exemptions for setup and deadline assertions; production lint policy remains unchanged. Every member inherits shared lints. rustfmt uses the pinned component's defaults.

## Browser assets

First install browser dependencies. Run node crates/ruphish-ui/web/build.mjs before direct WASM checks; Trunk.toml runs the same build-only generator automatically before compiling the UI and creates asset-manifest.json in staging afterward. The native server verifies every file against this manifest before readiness. This is asset generation, not a wrapper for checks. esbuild produces crates/ruphish-ui/web/generated/editor.js and the separate visual-frame JS/CSS/legal assets from the exact npm lockfile. The source-editor module must exist before WASM compilation because wasm-bindgen imports it; Trunk copies the visual assets for the isolated frame. Rebuild after changing editor sources or dependencies. Runtime package notices are preserved in web/THIRD-PARTY-NOTICES.txt and copied into the output; update them alongside dependency changes. No CDN or runtime npm install is used.

Trunk reads crates/ruphish-ui/index.html and its adjacent Cargo.toml, builds the wasm32-unknown-unknown binary using Cargo.lock, generates the matching wasm-bindgen JavaScript glue, and emits hashed files to dist/admin. Its wasm-bindgen tool is selected to match the locked crate, rather than maintained as a second manually synchronized version. Additional wasm-opt processing is explicitly disabled for this initial build; Rust release compilation and thin LTO still apply. Optimization/size decisions can be measured later.

Build with Trunk, then run the native executable and open its admin listener for functional UI testing. Account screens call the same-origin backend; standalone `trunk serve` has no configured authentication backend and cannot validate these flows. Never use stale dist output as proof of current source behavior; rebuild after UI changes. target/, dist/, local data/, .env and PERSONAL.md are ignored. Regression SQLite fixtures are deliberately not globally ignored.

Trunk 0.21.14 parses NO_COLOR as a boolean in both root and subcommand options, so NO_COLOR=1 can fail before a build starts even with a --no-color flag. Set `$env:NO_COLOR = 'true'` in PowerShell before direct Trunk commands, or use `NO_COLOR=true trunk build --release --locked` in a POSIX shell. This is a process-only value; do not change persistent environment settings.

## Dependency policy

deny.toml evaluates Windows/Linux native and browser graphs, explicitly allows reviewed permissive licenses and crates.io, rejects unknown registry/Git sources, and checks advisories/yanked packages. Duplicate versions are reported for review rather than globally denied because third-party version splits do not necessarily imply an actionable conflict. Avoid broad suppressions. No advisory ignore entries are configured. Network access is required for initial dependency/tool downloads and current advisory information; a network failure is a failed/unavailable check, not a clean audit.

Five exact-version license exceptions cover reviewed transitive dependencies: base16 0.2.1 (CC0), xxhash-rust 0.8.19 (Boost), matchit 0.8.4 (MIT AND BSD-3-Clause), subtle 2.6.1 (BSD-3-Clause), and dev-only webpki-root-certs 1.0.9 (CDLA-Permissive-2.0 trust data; retain its agreement if sharing that data). These satisfy the current allowlist without suppressing security advisories. CodeMirror and Jodit runtime packages are MIT; their notices are preserved in the browser output. cargo-deny does not inspect npm dependencies; npm audit checks their advisory graph separately.

GitHub actions/state, releases, publishing, distribution packaging, and deployment are deferred. Completing local checks does not authorize any of those actions.
