---
title: Validation
description: Select direct checks and understand the repository CI entry points and acceptance boundaries.
sidebar:
  order: 2
---

This is the command and scheduling reference for [agent completion rules](../../../AGENTS.md). Choose checks for the changed behavior and its consumers. Complete acceptance is a separate task, not a prerequisite for every local commit. Commands below run from the product repository, `ruphish/`; execute each command separately.

## Placement and cost

Cost describes the work performed, not a promised duration. Compilation, tool installation, asset generation and test execution are separate costs. CI step durations provide the measurements for tuning this policy; there is no measured CI baseline yet.

| Check | Guarantee and prerequisite | Cost and placement |
| --- | --- | --- |
| Rustfmt / Leptos formatting | Rust layout / Leptos markup layout; installed pinned tools | Small, no product compilation. Agents after applicable edits; CI on selected Rust/UI paths |
| Native Clippy | Native target compilation and lint policy, including tests | Compilation can dominate a cold run. Agents use affected targets; the product group checks all native targets |
| Native unit and ordinary integration tests | Domain, serialization, adapter, HTTP and SQLite behavior | Reuse compilation; filesystem/database cases cost more than pure functions. Agents select regressions; the product group runs the complete non-ignored native suite |
| JS tests and syntax | Actual editor/state/build transformations; npm dependencies where imported | Installation is separate. Run affected tests locally and all browser functional tests on product PRs. Syntax-only commands are needed only for files not parsed by tests/build |
| Native UI tests and WASM Clippy/Fetch tests | Pure UI transformations plus target-specific browser types and request construction | WASM compilation/test runner are separate from native compilation. The product group always runs browser checks; native checks cannot replace them |
| Development distribution | Actual generated JS/CSS/WASM and manifest | Often expensive when cold; reuse only matching inputs. Required for asset changes and executable acceptance prerequisites |
| Executable workflows | Real startup, HTTP, persistence, SMTP, participant privacy, restart/shutdown | The product group runs the complete non-ignored process suite in the same Linux job as native tests and Clippy, reusing its Cargo target directory |
| Server-provider contracts | Real MySQL/PostgreSQL SQL, locking, transactions, TLS and recovery | Docker setup plus compilation plus server execution. The persistence group runs both engines and both supported versions with a fixed matrix |
| Historical migrations and schema integrity | Independent legacy fixtures, preservation, drift rejection, rollback/reopen | Replays many schemas and costs substantially more. SQLite histories run with product checks; server histories run with persistence checks. Full nightly and release acceptance include both |
| Full provider executable workflows | Combined HTTP, delivery, cross-process ownership and recovery on each engine | Long integration scenarios. The persistence group includes these scenarios on both supported versions, including when shared helpers change |
| Windows/macOS native acceptance | Platform-specific compilation, unit/CLI and asset-promotion behavior | The product group runs complete native acceptance on Windows x64 and macOS ARM64, including platform asset promotion; Linux runs native and executable checks together |
| Public Docker acceptance | Controlled DNS/TLS/SMTP and privacy through the canonical fixture | Container startup and a Linux development build. Public fixture or shared build/workflow changes and full nightly/release acceptance |
| Dependency policy | Rust advisories/licenses/sources/bans and npm advisories | Tool install and advisory download are separate. Dependency/policy changes plus nightly; cached test success does not replace fresh advisory data |
| Bounded audience measurement | Existing ignored launch/read/claim/compose workload at legacy audience sizes | Measurement, not a wall-clock assertion. Full nightly/release acceptance; not an agent completion check |
| Workflow lint, gate and fixture initialization tests | YAML/Actions expressions, failure/skip handling and caller-owned Cargo artifact directories | Small compared with product builds. Every PR and after workflow edits; run `node --test .github/scripts/validation.test.mjs tests/acceptance/init.test.mjs` and inspect representative path groups when their patterns change |

Executable campaign tests use an isolated DNS/HTTPS fixture on ordinary loopback and an allocated port. The fixture belongs to the test workspace, so its origin remains stable across application restarts. No privileged-port configuration is needed. API and lifecycle process tests use small synthetic assets with a valid manifest; they exercise the real server without loading the full UI on each startup. Dedicated distribution-serving, immutability, traversal/integrity and symlink tests retain copies of the real built UI. Migration-only tests omit UI assets and inspect preserved data after shutdown; dedicated shared-database scenarios retain concurrent access. Process failures include bounded OS process observations and workspace file metadata, without dumping configuration or database contents.

The executable participant-privacy workflow runs the served JavaScript collector with Node and JSDOM. Run `npm ci --prefix crates/ruphish-ui/web` before executable tests on a fresh checkout, including jobs that download a prebuilt UI distribution; that distribution does not contain test dependencies.

## Prerequisites

Use the Rust toolchain/components/target in [rust-toolchain.toml](../../../rust-toolchain.toml). Tool versions are owned by [Cargo.toml](../../../Cargo.toml) workspace metadata and [Trunk.toml](../../../Trunk.toml); [development setup](../setup.md) explains formatting, generation and editor configuration. CI additionally pins product Node and tool installers in the [toolchain action](../../../.github/actions/toolchain/action.yml). The website job reads its Node version from [website/package.json](../../../website/package.json).

For a fresh browser setup, install the matching tools and npm dependencies. These are setup commands, not checks to repeat after every edit:

```sh
rustup show
cargo install leptosfmt --version 0.1.33 --locked --profile dev
cargo install trunk --version 0.21.14 --locked --profile dev
cargo install wasm-bindgen-cli --version 0.2.129 --locked --profile dev
cargo install cargo-deny --version 0.19.9 --locked --profile dev
npm ci --prefix crates/ruphish-ui/web
```

Keep local heavy work sequential. In PowerShell, set these values in the invoking process; Trunk requires the literal boolean `true` for `NO_COLOR`:

```powershell
$env:CARGO_BUILD_JOBS = '1'
$env:RUST_TEST_THREADS = '1'
$env:TOKIO_WORKER_THREADS = '2'
$env:RAYON_NUM_THREADS = '2'
$env:NO_COLOR = 'true'
```

On POSIX shells use `export CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 TOKIO_WORKER_THREADS=2 RAYON_NUM_THREADS=2 NO_COLOR=true`. Docker checks require Linux containers and Compose v2. Use only [the canonical acceptance fixture](../../../tests/acceptance/compose.yaml); initialize it with `node tests/acceptance/lab.mjs init` before first use. This preserves existing local application data and credentials.

## Checks after changes

For Rust changes, run formatting and affected-target Clippy/tests. For example, an account rule change starts with its regression and affected crate; add consumers and provider/process coverage when its behavior reaches those boundaries:

```sh
cargo fmt --all -- --check
cargo clippy -p ruphish-domain --all-targets --locked -- -D warnings
cargo test -p ruphish-domain --locked
```

For markup changes also run `leptosfmt --check crates/ruphish-ui/src crates/ruphish-ui/tests`. For JS changes use the relevant file under `crates/ruphish-ui/web/tests/`; for standalone files not parsed by those tests/build use `node --check <changed-file>`. Do not run every syntax-only command after a build that already parsed the same modules.

Executable cases are independently selectable through the existing `startup_process` target. Use the appropriate filter, not the full suite by default:

```sh
cargo test -p ruphish --test startup_process auth_process:: --locked -- --nocapture --test-threads=1
cargo test -p ruphish --test startup_process campaign_process:: --locked -- --nocapture --test-threads=1
cargo test -p ruphish --test startup_process resource_process:: --locked -- --nocapture --test-threads=1
cargo test -p ruphish --test startup_process public_process:: --locked -- --nocapture --test-threads=1
cargo test -p ruphish --test startup_process reporting_process:: --locked -- --nocapture --test-threads=1
cargo test -p ruphish --test startup_process overview_process:: --locked -- --nocapture --test-threads=1
```

`campaign_process::proxy_process::` selects just proxy scenarios. Check the number of executed tests: a misspelled Rust filter can succeed with zero tests. For exact test names add `--exact` after `--`. These focused API/workflow scenarios use synthetic assets and do not require a UI build. The dedicated distribution tests and the complete process suite require the real current `dist/admin` distribution. CI runs the complete non-ignored process suite for the product group; these filters remain useful for focused local regressions.

For SQL changes select the corresponding real-provider test in `database_providers`; use the fixture setup and direct commands in [MySQL verification](database-providers.md#mysql) or [PostgreSQL verification](database-providers.md#postgresql). MySQL transport/credentials/lock-timeout checks can now run independently of historical migrations:

```sh
cargo test -p ruphish-adapters --test mysql_preflight mysql_preflight_transport --locked -- --ignored --nocapture --test-threads=1
```

The CI persistence group runs complete provider suites, including historical upgrades and schema recovery, on both supported versions. There are no filename-based migration exceptions or `--skip` lists. Agents still select focused local scenarios based on semantics; the broad CI groups do not replace inspecting changed behavior.

## Public demo

The default UI build is the production application. Its existing tests continue to run without the opt-in `demo` feature. Demo tests are separate; never run production Fetch expectations against simulated responses. See [the local preview instructions](../../../demo/README.md) and [the demo development guide](../demo.md).

For the shared dataset and demo transport, use the functional and real-contract checks:

```sh
node --test --test-concurrency=1 demo/tests/api.test.mjs demo/tests/preview.test.mjs
cargo test -p ruphish-ui --test demo_contracts --features demo --locked
cargo clippy -p ruphish-ui --test demo_contracts --features demo --locked -- -D warnings
```

For the replacement SQL seeder, first build a normal UI distribution and the actual executable, then run the disposable SQLite scenario. This initializes current migrations, seeds once, restarts, reads affected API routes, checks outbound integrations are disabled, and rejects reseeding. It requires Node 26 for `node:sqlite` and does not use the acceptance lab:

```sh
trunk build --locked
cargo build -p ruphish --locked
node --test demo/tests/seed.test.mjs
```

For demo UI changes, apply the normal formatting rules, generate the demo transport, build the development artifact, and lint the WASM target:

```sh
node crates/ruphish-ui/web/build-demo.mjs
node --test demo/tests/bundle.test.mjs
trunk build --locked --features demo
cargo clippy -p ruphish-ui --target wasm32-unknown-unknown --features demo --locked -- -D warnings
```

When the shared transport changes, also run the existing `http.test.mjs` and default-feature `browser_requests` WASM target. The latter exercises the real Fetch path:

```sh
node --test crates/ruphish-ui/web/tests/http.test.mjs
cargo test -p ruphish-ui --test browser_requests --target wasm32-unknown-unknown --locked
cargo clippy -p ruphish-ui --target wasm32-unknown-unknown --locked -- -D warnings
```

Both local builds use `dist/.stage` and complete at `dist/admin`; rebuild the desired mode before using the output. CI builds demo assets in a separate job/cache and uploads `demo-distribution`, while native/provider tests consume only `admin-distribution`. Visual review and screenshot recapture remain user tasks.

## Complete source acceptance commands

This catalog is for an explicitly selected full acceptance run. It is not a local completion checklist. Generate the real development distribution before process tests; Trunk invokes the JS/CSS generator itself:

```sh
trunk build --locked
cargo fmt --all -- --check
leptosfmt --check crates/ruphish-ui/src crates/ruphish-ui/tests
cargo clippy --workspace --exclude ruphish-ui --all-targets --locked -- -D warnings
cargo test --workspace --exclude ruphish-ui --locked -- --nocapture --test-threads=1
node --test --test-concurrency=1 crates/ruphish-ui/web/tests/*.test.mjs
cargo test -p ruphish-ui --lib --tests --locked
cargo clippy -p ruphish-ui --lib --locked -- -D warnings
cargo test -p ruphish-ui --test '*' --target wasm32-unknown-unknown --locked
cargo clippy -p ruphish-ui --target wasm32-unknown-unknown --locked -- -D warnings
cargo clippy -p ruphish-ui --tests --target wasm32-unknown-unknown --locked -- -D warnings
npm audit --prefix crates/ruphish-ui/web
cargo deny check bans licenses sources advisories
```

When doing direct WASM checks without first building Trunk, generate imported inputs with `node crates/ruphish-ui/web/build.mjs`. Native executable tests build their executable; a separate `cargo build --workspace --exclude ruphish-ui --locked` is useful when an executable is needed outside those tests, not an additional proof after equivalent compilation. `cargo check -p ruphish-ui --target wasm32-unknown-unknown --locked` is an optional fast iteration command; omit it when the same target is already checked by Clippy.

The workspace command excludes explicitly ignored Docker/provider/performance cases. Complete acceptance additionally runs all direct commands for both supported versions in the MySQL/PostgreSQL guides, the [public fixture protocol test](local-fixture.md), and this measurement:

```sh
cargo test -p ruphish-adapters --test campaign_storage measure_launch_read_claim_and_compose_at_legacy_audience_sizes --locked -- --ignored --exact --nocapture --test-threads=1
```

Run native acceptance on Linux, Windows and macOS when validating the full supported platform behavior. The UI's Node WASM test proves request construction; it does not replace the user's manual browser appearance/interaction review. Root rewrite/specification checks remain separate: `py -3 tools/verify_specification.py` runs from the parent migration root only when those records change.

## GitHub selection and branch rules

Three entry points own schedules and branch rules. The reusable [validation workflow](../../../.github/workflows/validation.yml) owns source-check commands, and [paths.yml](../../../.github/paths.yml) owns the five broad path groups. Each group runs a fixed suite; there is no custom selector, feature-name inference, dynamic database matrix or smoke deduplication.

| File | Trigger | Validation | Required status |
| --- | --- | --- | --- |
| [pr.yml](../../../.github/workflows/pr.yml) | PR to main; push to main; manual full run | YAML path groups select fixed suites; a manual run selects all suites | PR gate on main |
| [nightly.yml](../../../.github/workflows/nightly.yml) | Daily at 02:23 UTC; manual | Full development-profile source acceptance, all provider versions and native platforms, histories/recovery, dependency policy and bounded performance measurement | Monitoring; does not replace PR gates |
| [release.yml](../../../.github/workflows/release.yml) | PR to release; manual | Full source acceptance, then optimized UI and native/executable tests on Linux x64, Windows x64 and macOS ARM64 | Release gate on release |
| [validation.yml](../../../.github/workflows/validation.yml) | Called by the three entry points | One implementation of source checks and an internal Validation gate | Internal prerequisite of the caller's gate |

The CI workflow always starts for PRs to main and pushes to main, including documentation and repository housekeeping, so its gate is reported. A commit-pinned `dorny/paths-filter` action reads the YAML groups and compares the checked-out PR merge commit against its base using Git. Pushes compare main against its revision before the push, including all commits in that push. Full checkout history avoids API file-count limits and extra permissions; deleted paths and both sides of renames participate. Nightly, release and manual runs pass `full: true` and bypass path filtering. The README CI badge tracks push runs on main: it reports the checks applicable to the latest push, while nightly runs cover the full suite.

| Group | Main paths | Fixed checks |
| --- | --- | --- |
| Website | `website/**`, `docs/**` | Website tests, Astro checks/build and site validation |
| Product | `crates/**`, `tests/**`, `migrations/**`, `demo/**`, `assets/website-demo/**`, runtime Docker inputs and shared build inputs | Normal browser tests/lint/formatting and development assets; separate demo contracts/build and shared seed checks; complete native Clippy/tests on Linux x64, Windows x64 and macOS ARM64; platform asset promotion |
| Persistence | Adapter/application/domain crates, `tests/**`, `migrations/**` and shared build inputs | Both supported MySQL and PostgreSQL versions, complete histories/schema recovery, transport checks and provider executable scenarios |
| Public | `tests/acceptance/**`, `Dockerfile`, `.dockerignore`, `docker/**` and shared build inputs | Canonical runtime image health, shutdown/persistence and DNS/TLS/SMTP/privacy fixture |
| Dependencies | Shared build inputs and browser npm manifests/lockfile | Cargo dependency policy and browser npm audit |

Shared build inputs include workspace/crate manifests, the Cargo lockfile, toolchain and formatter configuration, `.cargo/`, Trunk, dependency policy, line-ending policy, workflow/action implementations and CI path/gate files. Workflow and path/gate changes also select website checks. Issue templates, the PR template, CODEOWNERS and Dependabot configuration do not select product acceptance. The YAML file is authoritative; add new source/build locations to the appropriate group when introducing them.

These groups intentionally trade some extra execution for predictable coverage. For example, browser-only changes also run native/platform acceptance, and an adapter change runs both provider engines and versions. No existing test guarantee is moved to nightly by this selection simplification. The bounded audience measurement remains exclusive to full runs.

Linux native Clippy, library/integration tests, SQLite histories and executable scenarios share one job and Cargo target directory. Database jobs run independently of that job once browser assets are available; their fixed suite includes provider executable scenarios that consume those assets. Provider caches are separate from the Linux native cache so concurrent jobs cannot prevent its final test outputs from being saved. Browser compilation and each additional native platform still require their own compilation; this is not a single portable build artifact.

CI uses the tools' default build, test and worker parallelism. Provider and platform matrices have no repository-imposed parallel-job cap; GitHub schedules them within its hosted-runner limits. The bounded commands above are for local work. macOS CI covers Apple Silicon (ARM64); Intel macOS is no longer a supported validation target. Superseded runs for the same PR or scheduled ref are cancelled. Matching UI inputs can reuse development assets; tests still run. Optimized UI is built only in the release workflow. The final gates reject failed/cancelled checks, missing group outputs and unexpectedly skipped selected jobs.

Native platform and release test steps point `TMPDIR`, `TMP` and `TEMP` at GitHub's `runner.temp` directory. This keeps disposable databases and other fixtures in the job's temporary storage instead of the Windows user-profile directory. OS process metrics are optional failure diagnostics; an unavailable or timed-out metrics command does not itself fail a product test.

Actions use pinned commits, read-only repository permissions and checkout without persisted credentials. These workflows neither receive deployment secrets nor publish releases. Validate workflow edits locally with:

```sh
node --test .github/scripts/validation.test.mjs
actionlint -shellcheck="" -pyflakes=""
```

The gate tests execute the workflow's actual inline gate with successful, failed, cancelled, skipped and incomplete results. When path groups change, also inspect representative documentation, UI, shared Rust, persistence/migration, test-helper, build-configuration and housekeeping changes, including deleted/renamed files. Validate matching with the pinned path-filter action; workflow lint alone does not prove path coverage. Workflow-only changes do not require running product acceptance locally.

## Release-profile boundary

The release workflow validates optimized code and real optimized UI assets with the existing functional/process suites. Its UI artifact is an internal test input, not a shipping package. The commands are explicit and reserved for the release workflow or an explicitly requested local release-profile task:

```sh
trunk build --release --locked
cargo test --release --workspace --exclude ruphish-ui --locked -- --nocapture --test-threads=1
```

The intended distribution formats are a single release binary and a Docker image. Native release-profile suites run on Linux, Windows and macOS; real database/DNS/TLS/SMTP acceptance uses the canonical Linux Docker fixture in the preceding full source checks.

> TODO before publication: Complete asset embedding/bundling, define supported distribution platforms, implement artifact integrity and applicable signing/notarization, publish release binaries and the Docker image, and verify the resulting artifacts. Source acceptance alone does not verify the distribution artifacts.

Report commands actually executed, exit status and limitations. Separate setup/compile failures from failed executed tests, and both from ignored or unrun tests. A stable foundation changes when checks run, not whether their guarantees matter. No hosted workflow or product acceptance result is implied by local workflow lint.
