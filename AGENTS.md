# Product development instructions

## Scope and structure

The seven crate boundaries are domain, wire contracts, application use cases, adapters, HTTP, WASM UI, and executable composition. Keep domain rules authoritative, handlers/adapters thin, dependencies directed inward, and browser contracts free of backend secrets or infrastructure. Workspace architecture tests enforce the declared boundary policy; update that policy only for an intentional architectural change.

Native libraries are scaffolds until functionality is implemented with its tests; do not add fake domain models, no-op operations, or success-returning placeholder handlers. Existing compatibility fixtures are independent expectations, not evidence that Rust already supports those features. Preserve the legacy-test map's honest statuses and all original scenario guarantees as they are implemented.

## Working discipline

- Inspect relevant implementation, fixtures, consumers, and error paths before changing behavior. Follow complete workflows through validation, persistence, emitted effects, and recovery.
- Each invariant, transition, relationship, and derived value has one authoritative owner. Avoid duplicated state, redundant validation, and alternative stale implementations.
- Use `cargo new`, `cargo add`, `cargo remove`, `cargo update`, `cargo install`, and `rustup` commands for crate creation, dependency/tool installation and changes; let the CLI write dependency version entries and lockfiles rather than adding dependencies by manually typing versions into manifests. Editing workspace metadata, profiles, feature configuration, and lint policy is normal configuration work.
- Preserve external wire/data/template contracts and meaningful negative guarantees. Record an intentional compatibility difference and its tests explicitly before adopting it.
- Match user-visible capabilities and outcomes; cleaner implementations and UI arrangements are allowed. Legacy components, markup structure, and harmless serialization formatting are not compatibility requirements. Define tests by what users can do and what their content does, preserving exact bytes only where they are part of a real contract.
- Prefer typed errors at library boundaries and contextual startup errors. Production code forbids unsafe code and accidental unwrap/expect/panic/exit/todo/unimplemented/dbg usage; tests have narrow Clippy exemptions for assertions and setup.
- Add tests for actual domain/functionality guarantees, persisted state, protocols, and real failure boundaries. Do not add mirror tests for trivial code or claim scaffold checks establish feature parity.
- The user performs manual visual/UI interaction tests. Compile and build WASM; do not add browser/screenshot automation by default.
- Never manually hard-wrap Markdown paragraphs or list items. Use one source line per paragraph or list item.

## Toolchain and verification

`rust-toolchain.toml` pins Rust 1.99.1, rustfmt, Clippy, and wasm32-unknown-unknown. Manifests inherit edition 2024 and supported Rust 1.99. Lower compiler versions have not been validated. Keep the application Cargo.lock and use locked checks. Native default members exclude the browser binary deliberately; native success is not WASM verification.

Build the real admin distribution before running the native process suite: npm ci --prefix crates/ruphish-ui/web, then trunk build --release --locked. Process tests intentionally use the real generated WASM/JavaScript assets and fail if they are absent. Run checks directly from this directory. Do not add a task runner, check script, Cargo alias layer, or npm wrapper around these commands. Keep the actual commands visible here and report their individual results.

After native Rust changes, run:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --exclude ruphish-ui --all-targets --locked -- -D warnings
cargo test --workspace --exclude ruphish-ui --locked
cargo build --workspace --exclude ruphish-ui --locked
```

After browser/contract changes, install browser dependencies, run functional tests, generate the editor module, then separately check, lint, and build the browser target. Run each command directly from the product root:

```sh
npm ci --prefix crates/ruphish-ui/web
node --check crates/ruphish-ui/web/build.mjs
node --check crates/ruphish-ui/web/state.mjs
node --check crates/ruphish-ui/web/editor.mjs
node --check crates/ruphish-ui/web/navigation.mjs
node --check crates/ruphish-ui/web/account-menu.mjs
node --check crates/ruphish-ui/web/template-html.mjs
node --check crates/ruphish-ui/web/visual-session.mjs
node --check crates/ruphish-ui/web/visual-frame.mjs
node --check crates/ruphish-ui/web/visual-shell.mjs
node --check crates/ruphish-ui/web/visual-controller.mjs
node --test crates/ruphish-ui/web/tests/*.test.mjs
cargo test -p ruphish-ui --lib --locked
cargo test -p ruphish-ui --test browser_requests --target wasm32-unknown-unknown --locked
cargo clippy -p ruphish-ui --lib --locked -- -D warnings
node crates/ruphish-ui/web/build.mjs
cargo check -p ruphish-ui --target wasm32-unknown-unknown --locked
cargo clippy -p ruphish-ui --target wasm32-unknown-unknown --locked -- -D warnings
cargo clippy -p ruphish-ui --test browser_requests --target wasm32-unknown-unknown --locked -- -D warnings
trunk build --release --locked
```

Trunk 0.21.14 expects NO_COLOR to be a boolean. If the invoking environment has NO_COLOR=1, set the process value to true before direct Trunk commands: `$env:NO_COLOR = 'true'` in PowerShell, or `NO_COLOR=true trunk build --release --locked` in a POSIX shell. Do not change persistent user/system environment settings.

The WASM request tests execute the production Fetch request builder in Node using wasm-bindgen-test-runner. Install the matching runner with cargo install wasm-bindgen-cli --version 0.2.129 --locked; keep its version aligned with the locked wasm-bindgen crate. The target runner in .cargo/config.toml is the Cargo test runtime, not a check wrapper. These tests require Node with native Fetch support and do not launch or automate a browser.

For dependency changes and the full workspace acceptance pass, run:

```sh
npm audit --prefix crates/ruphish-ui/web
cargo deny --version
cargo deny check bans licenses sources advisories
```

The required cargo-deny version is recorded in Cargo.toml workspace metadata; Trunk enforces its own exact version from Trunk.toml. Do not silently skip unavailable tools or failed checks. No application feature flags exist yet; document and check supported combinations as features are introduced.

CodeMirror, Jodit and their browser dependencies are pinned in crates/ruphish-ui/web/package-lock.json. Node/npm are build/test tools, not server dependencies. Run node crates/ruphish-ui/web/build.mjs before direct WASM checks. Trunk.toml explicitly runs that build-only generator before each Trunk build and writes a SHA-256 distribution manifest afterward; the server rejects mismatched assets. This generator builds assets only; all checks remain separate direct commands. Review npm package licenses and preserve runtime notices in web/THIRD-PARTY-NOTICES.txt when dependencies change; cargo-deny only checks Rust dependencies.

Browser output is generated in dist/admin. Do not hand-edit or check in target/ or dist/, and rebuild after UI changes.

Shared lints must be inherited by every member. Native and browser dependency graphs are checked separately. Dependency policy allows reviewed permissive licenses and crates.io sources, rejects unknown Git/registry sources and advisories/yanked packages, and reports duplicate versions for review. Do not silence a finding with broad exemptions; record narrow, reasoned exceptions only where necessary.

The UI build workspace and the server asset root must remain separate. Trunk owns `dist/` and stages under `dist/.stage/`; the post-build hook arranges completed assets under `admin/`, producing the server distribution at `dist/admin/`. Never configure Trunk to stage inside the served directory or teach the server to ignore build artifacts. The server continues to reject hidden/unexpected paths and mismatched manifests.

## Data and side effects

SQLite is the initial complete target; MySQL follows later. Database upgrades must preserve a recoverable original and be tested against independent legacy data. Fixtures contain synthetic credentials and must be copied before mutation. Product tests must not contact real recipients or external integration endpoints.

Do not operate on GitHub, create releases/tags, publish, package for distribution, or deploy unless explicitly requested. Stop only development processes started in the current session. Report commands actually run and any unresolved checks or reachable defects honestly. Optional PERSONAL.md in this directory adds local preferences if present.

## Git and commit messages

Read-only Git inspection such as `git status`, `git diff`, `git log`, and `git show` is allowed.

Commit and push only when the user asks for it in that turn, never as a side effect of finishing a task. One approval covers that commit or push only; it does not carry over to later ones.

Do not merge, rebase, create/delete/rename branches, switch branches, or rewrite history unless explicitly requested.

When writing or suggesting a commit message:

- Describe the net changes against `HEAD`, not the local implementation path.
- Use imperative mood, present tense, and concise language.
- Use the format `<type>(<scope>): <message>`; scope is optional. Allowed types are `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`, `ci`, and `revert`.
- Include 2–5 precise bullet points in the body.
- Do not mention temporary or uncommitted planning files unless they are part of the intended change.
- Do not add a `Co-Authored-By` trailer, a generated-by footer, or any other agent attribution.
