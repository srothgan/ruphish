---
title: Validation commands
description: Run focused local checks and find the broader checks needed for changed consumers and boundaries.
sidebar:
  order: 310
---

Run commands from the repository root unless a section says otherwise. Install the relevant tools through [development setup](../development-setup/index.md#install-tools-for-your-change). Select checks by behavior and affected consumers; a small change does not automatically require full acceptance.

## Select your checks

| Changed area | Start with | Add when affected |
| --- | --- | --- |
| Domain or contract | Rust formatting, crate Clippy, crate tests | HTTP, UI/WASM, provider, or process consumers |
| HTTP/application behavior | Focused crate and process tests | Authentication, persistence, external protocols, and recovery |
| SQL, locking, or migration | Provider regression and relevant SQLite tests | Both supported server versions, history conversion, process restart |
| UI Rust or browser-shared rules | UI formatting, native tests, WASM checks | Development distribution and mounted/manual UI scenarios |
| JavaScript or build tooling | Prettier, ESLint, affected functional tests | Regenerate/build output when generated behavior changes |
| Docs | Links, examples, rendered navigation | Website checks; no Rust build for prose alone |
| CI/release implementation | Script tests and workflow lint | The affected workflow's actual acceptance stages |

## Rust

For a domain change, for example:

```sh
cargo fmt --all -- --check
cargo clippy -p ruphish-domain --all-targets --locked -- -D warnings
cargo test -p ruphish-domain --locked
```

Replace the crate with the affected crate and add relevant consumers. To apply formatting, run `cargo fmt --all`. `cargo check` is useful during iteration; do not repeat it after Clippy has already checked the same target without a new reason.

## Executable workflows

The `startup_process` target includes independently selectable modules. Choose a relevant filter:

```sh
cargo test -p ruphish --test startup_process auth_process:: --locked -- --nocapture
cargo test -p ruphish --test startup_process campaign_process:: --locked -- --nocapture
cargo test -p ruphish --test startup_process resource_process:: --locked -- --nocapture
cargo test -p ruphish --test startup_process public_process:: --locked -- --nocapture
cargo test -p ruphish --test startup_process reporting_process:: --locked -- --nocapture
```

Focused API/lifecycle cases use synthetic assets; distribution-serving cases and the complete suite need the real current `dist/admin/`. Install browser npm dependencies even when consuming a supplied distribution: participant-collector tests need Node/JSDOM, which the bundle does not contain. Use `-- --list` to inspect available cases, and check the executed count after filtering.

## Frontend and browser code

Format Leptos markup as well as Rust:

```sh
leptosfmt crates/ruphish-ui/src crates/ruphish-ui/tests
cargo fmt --all
leptosfmt --check crates/ruphish-ui/src crates/ruphish-ui/tests
cargo fmt --all -- --check
```

For direct WASM checks without a Trunk build, prepare imported modules first. Then run the applicable native/browser checks:

```sh
node crates/ruphish-ui/web/build.mjs
cargo test -p ruphish-ui --lib --tests --locked
cargo clippy -p ruphish-ui --lib --locked -- -D warnings
cargo test -p ruphish-ui --test '*' --target wasm32-unknown-unknown --locked
cargo clippy -p ruphish-ui --target wasm32-unknown-unknown --locked -- -D warnings
cargo clippy -p ruphish-ui --tests --target wasm32-unknown-unknown --locked -- -D warnings
```

`.cargo/config.toml` selects `wasm-bindgen-test-runner`. These Node-hosted WASM tests check their implemented browser contracts; use manual review for actual browser interaction.

### Mounted UI regressions

When changing forms, route owners, filters, or browser output, build the development distribution and exercise the mounted application:

```sh
trunk build --locked
node --test crates/ruphish-ui/web/tests/ui-interactions.artifact.mjs
```

This harness uses the real WASM bundle with synthetic responses. It checks state retention, navigation, and error recovery without contacting a real installation. It does not verify layout, the operating-system file picker, or sandbox enforcement. Select applicable [manual checks](manual-browser-checks.md) as well.

## JavaScript

Install root tooling with `npm ci --ignore-scripts` on a fresh checkout or after its dependency inputs change. Then:

```sh
npx --no-install prettier . --check
npx --no-install eslint . --max-warnings=0
node --test crates/ruphish-ui/web/tests/*.test.mjs
```

Choose affected functional tests locally instead of always running the entire browser directory. To deliberately apply formatting, use `npx --no-install prettier . --write`. `.prettierignore` limits this command to maintained JavaScript; it does not reformat Markdown, Rust, JSON, or YAML. ESLint already parses these files, so a separate syntax-only check is unnecessary.

The root npm manifest pins a `minimatch` override within ESLint's dependency tree. When updating that dependency, inspect whether the override is still necessary, regenerate the lockfile through npm, and validate the resulting tool installation and lint behavior.

## Starter templates

Use functional content tests and the native catalog workflow when changing bundled content:

```sh
node --test crates/ruphish-ui/web/tests/interactive-starters.test.mjs crates/ruphish-ui/web/tests/learning-starters.test.mjs
cargo test -p ruphish --test startup_process starter_templates_process:: --locked -- --nocapture
cargo test -p ruphish --test startup_process public_process::learning_pages:: --locked -- --nocapture
```

Inspect the rendered prepared preview after rebuilding the server. See [starter-template development](../development-guides/starter-templates.md) for content and attribution requirements.

## Public demo

For synthetic data and simulated responses:

```sh
node --test demo/tests/api.test.mjs demo/tests/sorting.test.mjs demo/tests/preview.test.mjs
cargo test -p ruphish-ui --test demo_contracts --features demo --locked
cargo clippy -p ruphish-ui --test demo_contracts --features demo --locked -- -D warnings
```

The disposable SQLite seeder test needs the real normal distribution and executable, and Node 26 for `node:sqlite`:

```sh
trunk build --locked
cargo build -p ruphish --locked
node --test demo/tests/seed.test.mjs
```

For demo browser output:

```sh
node crates/ruphish-ui/web/build-demo.mjs
node --test demo/tests/bundle.test.mjs
trunk build --locked --features demo crates/ruphish-ui/web/generated/index.html
node --test --test-name-pattern=campaign-wizard crates/ruphish-ui/web/tests/ui-interactions.artifact.mjs
cargo clippy -p ruphish-ui --target wasm32-unknown-unknown --features demo --locked -- -D warnings
```

Rebuild the normal distribution before using `dist/admin/` for the real server. When changing shared transport, also run the normal Fetch tests:

```sh
node --test crates/ruphish-ui/web/tests/http.test.mjs
cargo test -p ruphish-ui --test browser_requests --target wasm32-unknown-unknown --locked
```

## Database changes

Use [database-provider tests](database-provider-tests.md) for service preparation, environment variables, focused filters, supported-version runs, and cleanup. Historical conversion tests need their independent fixtures; a fresh-schema test alone cannot prove migration correctness.

## Documentation and website

Use the [documentation workflow](../development-guides/documentation-and-website.md#verify-a-documentation-change). Select Node 24.x and run website commands from `website/`. Root JavaScript checks still apply when changing maintained website scripts. A prose-only contribution does not require a product build.

## Full source acceptance

Use this broader set for an explicitly selected acceptance run. Prepare all relevant tools and browser dependencies, then:

```sh
trunk build --locked
npx --no-install prettier . --check
npx --no-install eslint . --max-warnings=0
cargo fmt --all -- --check
leptosfmt --check crates/ruphish-ui/src crates/ruphish-ui/tests
cargo clippy --workspace --exclude ruphish-ui --all-targets --locked -- -D warnings
cargo test --workspace --exclude ruphish-ui --locked -- --nocapture
node --test crates/ruphish-ui/web/tests/*.test.mjs
cargo test -p ruphish-ui --lib --tests --locked
cargo clippy -p ruphish-ui --lib --locked -- -D warnings
cargo test -p ruphish-ui --test '*' --target wasm32-unknown-unknown --locked
cargo clippy -p ruphish-ui --target wasm32-unknown-unknown --locked -- -D warnings
cargo clippy -p ruphish-ui --tests --target wasm32-unknown-unknown --locked -- -D warnings
node --test crates/ruphish-ui/web/tests/ui-interactions.artifact.mjs
cargo deny check bans licenses sources
```

These commands do not include ignored server/provider or measurement cases. Full acceptance also runs [both supported database versions](database-provider-tests.md), the [public fixture](local-acceptance-fixture.md), platform acceptance, and the bounded workload:

```sh
cargo test -p ruphish-adapters --test campaign_storage measure_launch_read_claim_and_compose_at_legacy_audience_sizes --locked -- --ignored --exact --nocapture
```

The workload is a measurement, not a wall-clock pass/fail promise. [CI workflows](../maintenance/ci-workflows.md) defines which full suites run on each platform. [Release validation](../maintenance/releases.md) additionally verifies optimized code and packaged artifacts. Record exactly which portions ran; do not describe a partial local selection as full acceptance.
