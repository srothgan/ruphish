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
| Native Clippy | Native target compilation and lint policy, including tests | Compilation can dominate a cold run. Agents use affected targets; native PR baseline checks all native targets |
| Native unit and ordinary integration tests | Domain, serialization, adapter, HTTP and SQLite behavior | Reuse compilation; filesystem/database cases cost more than pure functions. Agents select regressions; native PR baseline runs ordinary contracts |
| JS tests and syntax | Actual editor/state/build transformations; npm dependencies where imported | Installation is separate. Run affected tests locally and all browser functional tests on UI PRs. Syntax-only commands are needed only for files not parsed by tests/build |
| Native UI tests and WASM Clippy/Fetch tests | Pure UI transformations plus target-specific browser types and request construction | WASM compilation/test runner are separate from native compilation. Required for affected UI/shared contracts; native checks cannot replace them |
| Development distribution | Actual generated JS/CSS/WASM and manifest | Often expensive when cold; reuse only matching inputs. Required for asset changes and executable acceptance prerequisites |
| Executable smoke and feature workflows | Real startup, HTTP, persistence, SMTP, participant privacy, restart/shutdown | Process startup and real asset verification add CPU/I/O. UI PRs run four smoke scenarios; native PRs add matching feature groups. Unclassified runtime changes run the full process suite |
| Server-provider contracts | Real MySQL/PostgreSQL SQL, locking, transactions, TLS and recovery | Docker setup plus compilation plus server execution. Relevant engine on ordinary provider PRs; both engines for shared domain/application/storage changes |
| Historical migrations and schema integrity | Independent legacy fixtures, preservation, drift rejection, rollback/reopen | Replays many schemas and costs substantially more. Affected migration/schema changes, full nightly and release-branch acceptance |
| Full provider executable workflows | Combined HTTP, delivery, cross-process ownership and recovery on each engine | Long integration scenarios. Changes to provider executable scenarios/fault proxies, plus full nightly and release-branch acceptance on both supported versions |
| Windows/macOS native acceptance | Platform-specific compilation, unit/CLI and asset-promotion behavior | Native PRs run Clippy and ordinary functionality. Platform-sensitive changes and nightly run complete native acceptance on Windows x64, macOS ARM64 and macOS x64; Linux runs the main baseline |
| Public Docker acceptance | Controlled DNS/TLS/SMTP and privacy through the canonical fixture | Container startup and a Linux development build. Fixture changes and full nightly/release acceptance |
| Dependency policy | Rust advisories/licenses/sources/bans and npm advisories | Tool install and advisory download are separate. Dependency/policy changes plus nightly; cached test success does not replace fresh advisory data |
| Bounded audience measurement | Existing ignored launch/read/claim/compose workload at legacy audience sizes | Measurement, not a wall-clock assertion. Full nightly/release acceptance; not an agent completion check |
| Workflow lint and selector tests | YAML/Actions expressions and selection boundaries | Small compared with product builds. Every PR and after workflow edits |

## Prerequisites

Use the Rust toolchain/components/target in [rust-toolchain.toml](../../../rust-toolchain.toml). Tool versions are owned by [Cargo.toml](../../../Cargo.toml) workspace metadata and [Trunk.toml](../../../Trunk.toml); [development setup](../setup.md) explains formatting, generation and editor configuration. CI additionally pins Node and tool installers in the [toolchain action](../../../.github/actions/toolchain/action.yml).

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

`campaign_process::proxy_process::` selects just proxy scenarios. Check the number of executed tests: a misspelled Rust filter can succeed with zero tests. For exact test names add `--exact` after `--`. These process tests require the real current `dist/admin` distribution. The CI smoke names are listed in the [selector](../../../.github/scripts/select.mjs), and tested against their source declarations.

For SQL changes select the corresponding real-provider test in `database_providers`; use the fixture setup and direct commands in [MySQL verification](database-providers.md#mysql) or [PostgreSQL verification](database-providers.md#postgresql). MySQL transport/credentials/lock-timeout checks can now run independently of historical migrations:

```sh
cargo test -p ruphish-adapters --test mysql_preflight mysql_preflight_transport --locked -- --ignored --nocapture --test-threads=1
```

Ordinary CI provider runs omit historical upgrade/schema-recovery scenarios using explicit `--skip` arguments in the [workflow](../../../.github/workflows/validation.yml). Migration changes run the complete provider suite on both supported versions. Do not copy the omissions into a migration verification task. Agents select relevant scenarios based on semantics; CI path selection is a conservative fallback, not a replacement for inspecting changed behavior.

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

Three entry-point files own the schedules and branch rules. A fourth reusable workflow owns the check commands, so the three entry points cannot drift into different copies of the same suite.

| File | Trigger | Validation | Required status |
| --- | --- | --- | --- |
| [pr.yml](../../../.github/workflows/pr.yml) | PR to main; manual profiles | Changed-path selection, Linux baseline, affected browser/provider/workflow checks, Windows and macOS native checks | PR gate on main |
| [nightly.yml](../../../.github/workflows/nightly.yml) | Daily at 02:23 UTC; manual | Full development-profile source acceptance, all supported provider versions, histories/recovery, all native platforms, dependency policy and bounded performance measurement | Monitoring; does not replace PR gates |
| [release.yml](../../../.github/workflows/release.yml) | PR to release/** or prod; manual | Full source acceptance, then optimized UI and native/executable tests on Linux x64, Windows x64, macOS ARM64 and macOS x64 | Release gate on release/** and prod |
| [validation.yml](../../../.github/workflows/validation.yml) | Called by the three entry points | One authoritative implementation of source checks and their internal Validation gate | Internal prerequisite of the caller's gate |

PR workflows always start, including documentation-only changes, so their final gate is reported. Selection uses the complete PR merge-base diff, including deleted paths and both sides of renames. Unrecognized non-documentation inputs, workspace manifests, lockfiles, toolchain/policy configuration and workflow changes select full source acceptance. UI paths select browser checks and smoke; domain/contracts include their native/browser consumers. Provider paths select their engine; shared adapter/application/domain changes select both engines. Schema, migration, upgrade implementation and fixture changes select historical checks and both supported versions of affected engines. Editing bundled provider regressions also runs their historical cases rather than skipping tests within the changed file.

Changes to MySQL/PostgreSQL executable scenarios or their protocol fault proxies select the corresponding real provider executable workflow; changing the shared server workflow selects both. Ordinary adapter integration targets are explicitly classified and checked against the commands that execute them. A new unclassified integration target selects full acceptance. UI integration tests use Cargo's quoted '*' target pattern to include newly added targets. Startup/config/TLS/transport changes additionally select full Windows/macOS process acceptance; ordinary native PRs use the smaller cross-platform baseline. Required process filters retain four smoke scenarios without repeating cases already covered by a selected feature group.

Manual profiles in pr.yml are auto (full without a PR diff), native (native baseline plus smoke), ui (browser plus smoke), databases (both engines/versions including histories, without provider executable workflows), and full. Real PRs always use their diff. Nightly and release entry points always request full source acceptance; neither accepts a reduced manual profile. A manual provider-only run does not require browser assets. Profiles change selected jobs, not the commands within those jobs.

CI limits Cargo to one build job and Rust tests to one thread. Provider matrices run one entry at a time; platform matrices run at most two. Independent jobs use separate hosted runners and do not consume the developer's CPU. Superseded runs for the same PR or scheduled ref are cancelled. Compilation/tool caches are isolated by OS, architecture and purpose. Matching UI inputs can reuse development assets; optimized UI is built in the release workflow. Caches accelerate setup and never count as passing tests. Final gates reject failed, cancelled or unexpectedly skipped selected work.

Actions use pinned commits, read-only repository permissions and checkout without persisted credentials. These workflows neither receive deployment secrets nor publish releases. Validate workflow edits locally with these small checks:

```sh
node --test .github/scripts/select.test.mjs
actionlint -shellcheck="" -pyflakes=""
```

## Release-profile boundary

The release workflow validates optimized code and real optimized UI assets with the existing functional/process suites. Its UI artifact is an internal test input, not a shipping package. The commands are explicit and reserved for the release workflow or an explicitly requested local release-profile task:

```sh
trunk build --release --locked
cargo test --release --workspace --exclude ruphish-ui --locked -- --nocapture --test-threads=1
```

The intended distribution formats are a single release binary and a Docker image. Native release-profile suites run on Linux, Windows and macOS; real database/DNS/TLS/SMTP acceptance uses the canonical Linux Docker fixture in the preceding full source checks.

> TODO before publication: Complete asset embedding/bundling, define supported distribution platforms, implement artifact integrity and applicable signing/notarization, publish release binaries and the Docker image, and verify the resulting artifacts. Source acceptance alone does not verify the distribution artifacts.

Report commands actually executed, exit status and limitations. Separate setup/compile failures from failed executed tests, and both from ignored or unrun tests. A stable foundation changes when checks run, not whether their guarantees matter. No hosted workflow or product acceptance result is implied by local workflow lint.
