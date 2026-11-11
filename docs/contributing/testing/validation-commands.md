---
title: Validation commands
description: Run focused local checks and find the broader checks needed for changed consumers and boundaries.
sidebar:
  order: 310
---

This page is a catalog of checks, their coverage, and command examples for contributors and coding agents. Its command blocks are not an execution checklist. Run commands from the repository root unless a section says otherwise. Install only the tools needed for the selected work through [development setup](../development-setup/index.md#install-tools-for-your-change).

## Select your checks

Select a check when it can detect a plausible defect introduced by the change and adds useful evidence beyond completed checks. Choose the smallest test target, module, or named scenario that covers that guarantee. A shared helper or component does not by itself justify every consumer's suite. Unrelated, redundant, or disproportionate checks can be omitted; document the actual verification and material limits. Ordinary edits, commits, and frontend builds do not select full acceptance.

| Changed guarantee | Useful evidence | Selection boundary |
| --- | --- | --- |
| Domain rule or serialized contract | Relevant Rust formatting/lint and focused rule or serialization regressions | Consumer tests when the changed contract affects their behavior |
| HTTP/application behavior | A focused request or process regression | The affected authorization, persistence, protocol, or recovery boundary |
| SQL, locking, or migration | A focused adapter/provider regression | Relevant providers and versions; expand only for the database guarantees changed |
| UI state or browser-shared rule | Formatting and a relevant target-specific lint/compilation check; a focused logic or mounted regression | Native logic, a browser contract, or a named workflow according to what changed |
| UI appearance or assets | Development output for manual review | Appearance alone does not select state/request suites; the user owns manual review during agent-assisted work |
| JavaScript or build tooling | Relevant formatting/lint and selected functional tests | The changed module or generated-output contract |
| Documentation | Relevant links, examples, and consistency checks | Site checks for changed rendering, navigation, or generation behavior; no product build for prose alone |
| CI/release implementation | Affected script tests or workflow lint | The workflow behavior changed; editing it does not automatically select every acceptance stage |

After edits settle, reuse successful results unless a later change affects their evidence. A development build already checks compilation of its target; an additional `cargo check` does not add that evidence again. Clippy adds lint coverage and tests add their asserted behavior, so select them for those purposes. Avoid competing Cargo operations against one output directory and rebuild only when changed inputs or the requested artifact require it.

## Rust

For a domain settings rule, these commands offer formatting, library lint, and the focused settings integration target. Select the evidence needed for the particular change:

```sh
cargo fmt --all -- --check
cargo clippy -p ruphish-domain --lib --locked -- -D warnings
cargo test -p ruphish-domain --test settings --locked
```

Choose the crate and test target for the changed rule; use a module or test-name filter when it narrows a larger target meaningfully. To apply Rust formatting, run `cargo fmt --all`. `cargo check` can provide compilation evidence during iteration when a build or Clippy has not already checked that target.

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

For changed Leptos markup, select the changed file or files for markup formatting as well as Rust formatting. For example:

```sh
leptosfmt --check crates/ruphish-ui/src/settings/page.rs
cargo fmt --all -- --check
```

To apply markup formatting, omit `--check`; to apply Rust formatting, use `cargo fmt --all`. Replace the example file with the files changed.

The following are separate checks with different coverage, not consecutive steps:

| Check | When it adds relevant evidence | Example command |
| --- | --- | --- |
| Native UI logic tests | A platform-independent state or transformation rule changed; choose its test target and filter | `cargo test -p ruphish-ui --test form_workflows --locked` |
| Native UI library Clippy | Code compiled into the native UI library changed | `cargo clippy -p ruphish-ui --lib --locked -- -D warnings` |
| Browser-target Clippy | Browser-only Rust or shared code needs WASM lint/compilation evidence | `cargo clippy -p ruphish-ui --target wasm32-unknown-unknown --locked -- -D warnings` |
| A WASM contract target | The browser contract asserted by that test target changed; this example covers requests | `cargo test -p ruphish-ui --test browser_requests --target wasm32-unknown-unknown --locked` |
| WASM test Clippy | WASM test code changed and needs lint coverage | `cargo clippy -p ruphish-ui --test browser_requests --target wasm32-unknown-unknown --locked -- -D warnings` |

Native tests do not exercise browser-only Rust, and passing native tests is not a reason to run every WASM target. For a selected standalone WASM check, prepare missing or stale imported modules with `node crates/ruphish-ui/web/build.mjs`. Trunk performs that generation itself; an additional generator run is unnecessary for the same build inputs.

`.cargo/config.toml` selects `wasm-bindgen-test-runner`. These Node-hosted WASM tests check their implemented browser contracts; use manual review for actual browser interaction.

### Development output

Build when a runnable artifact is needed, including when the user needs to try frontend changes. After implementation edits settle, the normal development build is:

```sh
trunk build --locked
```

The completed application is `dist/admin/`. Reuse that output for selected mounted checks while its inputs remain unchanged. Building an artifact does not select additional test suites. During agent-assisted work, manual visual/interaction review belongs to the user.

### Mounted UI regressions

A named mounted scenario can verify a changed workflow such as confirmation, draft retention, navigation, or recovery. Choose the scenario that asserts the changed guarantee. Browser output changes alone do not require the mounted suite. With a current development build, this example selects the company-settings feedback workflow:

```sh
node --test --test-name-pattern=settings-feedback crates/ruphish-ui/web/tests/ui-interactions.artifact.mjs
```

Replace the example pattern with the relevant named scenarios. The harness uses the real WASM bundle with synthetic responses without contacting a real installation. It does not verify layout, the operating-system file picker, or sandbox enforcement; [manual checks](manual-browser-checks.md) describe those boundaries. The unfiltered suite is documented under [full source acceptance](#full-source-acceptance).

An isolated build can intentionally preserve a running local server's distribution: use `trunk build --locked --dist target/ui-validation/dist` and set `RUPHISH_UI_DIST` to `target/ui-validation/dist/admin` for the selected mounted command. The fixture resolves this path from the repository working directory; unset the variable afterward. This does not update the user's normally served frontend and does not replace delivering `dist/admin/` when they need to try the changes. Isolated builds retain Trunk's `.stage/` staging and the completed `admin/` subtree.

## JavaScript

Install root tooling with `npm ci --ignore-scripts` on a fresh checkout or after its dependency inputs change. Formatting/lint of the changed module and its focused regression are separate choices. For a navigation-module change, examples are:

```sh
npx --no-install prettier crates/ruphish-ui/web/navigation.mjs --check
npx --no-install eslint crates/ruphish-ui/web/navigation.mjs --max-warnings=0
node --test crates/ruphish-ui/web/tests/navigation.test.mjs
```

Substitute the changed files and relevant test files rather than running the entire browser directory. To apply formatting, use `--write` for the selected files. `.prettierignore` limits formatting to maintained JavaScript; it does not reformat Markdown, Rust, JSON, or YAML. ESLint already parses these files, so a separate syntax-only check is unnecessary.

The root npm manifest pins a `minimatch` override within ESLint's dependency tree. When updating that dependency, inspect whether the override is still necessary, regenerate the lockfile through npm, and validate the resulting tool installation and lint behavior.

## Starter templates

For changed bundled content, select the functional content tests that cover its controls or behavior. Select the native catalog or process target when its preparation, persistence, or serving contract changed. Available examples are:

```sh
node --test crates/ruphish-ui/web/tests/interactive-starters.test.mjs crates/ruphish-ui/web/tests/learning-starters.test.mjs
cargo test -p ruphish --test startup_process starter_templates_process:: --locked -- --nocapture
cargo test -p ruphish --test startup_process public_process::learning_pages:: --locked -- --nocapture
```

Manual inspection of prepared previews can establish rendering behavior when that is the changed guarantee; a content edit alone does not require rebuilding the server or running every catalog workflow. See [starter-template development](../development-guides/starter-templates.md) for content and attribution requirements.

## Public demo

Choose checks for the demo boundary changed. These examples cover simulated API/data behavior and demo contract code; they are not required merely because shared UI code changed:

```sh
node --test demo/tests/api.test.mjs demo/tests/sorting.test.mjs demo/tests/preview.test.mjs
cargo test -p ruphish-ui --test demo_contracts --features demo --locked
cargo clippy -p ruphish-ui --test demo_contracts --features demo --locked -- -D warnings
```

When the disposable SQLite seeder's behavior needs verification, its test needs the real normal distribution and executable, and Node 26 for `node:sqlite`. Reuse current artifacts; build missing or stale ones:

```sh
trunk build --locked
cargo build -p ruphish --locked
node --test demo/tests/seed.test.mjs
```

For demo browser output, select from these commands according to the output or contract needed:

| Purpose | Command |
| --- | --- |
| Generate the demo entry and imported assets | `node crates/ruphish-ui/web/build-demo.mjs` |
| Verify the generated demo bundle contract | `node --test demo/tests/bundle.test.mjs` |
| Build a runnable demo distribution | `trunk build --locked --features demo crates/ruphish-ui/web/generated/index.html` |
| Exercise the mounted demo campaign wizard | `node --test --test-name-pattern=campaign-wizard crates/ruphish-ui/web/tests/ui-interactions.artifact.mjs` |
| Lint changed demo-feature Rust on its actual target | `cargo clippy -p ruphish-ui --target wasm32-unknown-unknown --features demo --locked -- -D warnings` |

Rebuild the normal distribution before using `dist/admin/` for the real server after a demo build. For changed transport, normal Fetch tests provide evidence that simulated demo responses cannot supply. Choose the target that covers the changed boundary:

```sh
node --test crates/ruphish-ui/web/tests/http.test.mjs
cargo test -p ruphish-ui --test browser_requests --target wasm32-unknown-unknown --locked
```

## Database changes

Use [database-provider tests](database-provider-tests.md) for service preparation, environment variables, focused filters, supported-version runs, and cleanup. Historical conversion tests need their independent fixtures; a fresh-schema test alone cannot prove migration correctness.

## Documentation and website

For prose changes, inspect relevant links, examples, and consistency against their source. The [documentation workflow](../development-guides/documentation-and-website.md#verify-a-documentation-change) also documents site checks for rendering, navigation, or generation changes; it is not a requirement to run the website suite for every text edit. For a selected website check, use Node 24.x and run its commands from `website/`. Select root JavaScript checks when maintained website scripts change. Prose-only changes do not require a product build or test suite.

## Full source acceptance

The following broad recipes document acceptance coverage. Use them for an explicitly selected acceptance task, not as follow-up steps for ordinary edits, commits, or development builds. Prepare the tools and dependencies for that selected coverage:

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
