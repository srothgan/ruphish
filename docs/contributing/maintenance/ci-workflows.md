---
title: CI workflows
description: Understand path-selected checks, full acceptance, failure gates, and workflow validation.
sidebar:
  order: 410
---

The [shared validation workflow](../../../.github/workflows/validation.yml) owns source checks. [The demo workflow](../../../.github/workflows/demo.yml) owns its separate static artifact. [Path groups](../../../.github/paths.yml) determine which fixed suites run for an ordinary change.

## Entry points

| Workflow | Trigger | Behavior |
| --- | --- | --- |
| `pr.yml` | Pull request to main, push to main, manual dispatch | Selects applicable source/demo checks; manual dispatch selects full checks |
| `pr-title.yml` | PR opened, edited, synchronized, or reopened against main | Checks the conventional title format |
| `nightly.yml` | Daily at 02:23 UTC or manual dispatch | Runs full source acceptance, including server versions and bounded measurement |
| `codeql.yml` | Pull request to main, push to main, Monday at 04:27 UTC, manual dispatch | Runs default CodeQL security queries for Rust, JavaScript/TypeScript, and GitHub Actions, excluding test-only files |
| `release.yml` | Push to main changing `Cargo.toml`, or manual dispatch | A workspace version increase selects full release validation, then creates the tag and publishes; unchanged versions skip release jobs and manual runs never publish |

CI starts even for documentation and housekeeping changes so its final gate can report a result. A documentation-only change does not select product acceptance unless it also changes a broader input. Website checks and deployment follow the website's separate Vercel configuration.

## Path-selected checks

| Group | Main inputs | Checks |
| --- | --- | --- |
| Product | Crates, product tests, demo data, runtime image files, shared build inputs | Browser checks/build, native Clippy/tests and executable workflows, Windows x64 and macOS ARM64 acceptance |
| Persistence | Domain/application/adapter crates, tests, migrations, shared build inputs | Both supported MySQL and PostgreSQL versions, histories, schema recovery, and provider executable scenarios |
| Public | Acceptance fixture, runtime configuration and Docker inputs, shared build inputs | Packaged image and controlled DNS/HTTPS/SMTP/privacy scenarios |
| Demo | UI/domain/contracts, starter content, demo data, build/deployment inputs | Demo contracts, browser checks, optimized build, and static deployment artifact |

Shared build inputs include manifests, lockfile, toolchain, formatter/dependency policy, Trunk configuration, workflow/action implementations, and release scripts. The YAML file is authoritative. Add new source/build locations there when required; do not construct commands dynamically from changed filenames.

The always-run quality job installs root npm tools, checks formatting/lint, runs workflow/fixture-initialization script tests, and lints workflows. Changes limited to script test files still exercise these checks without selecting product builds. Issue templates, CODEOWNERS, and Dependabot configuration do not select product acceptance by themselves.

The Rust dependency policy job also runs on every validation, independently of changed paths. It runs `cargo deny check advisories bans licenses sources`, so newly reported vulnerabilities are checked even when a PR does not change its dependencies. The validation gate requires this job to succeed; a failed, cancelled, missing or skipped dependency check blocks the PR gate.

The pinned path-filter action compares PR merge commits with their base using Git. Push selection uses changes since the prior main revision. Full checkout history supports deleted paths and both sides of renames. Nightly, release, and manual full runs bypass filtering.

## CodeQL security analysis

The [CodeQL workflow](../../../.github/workflows/codeql.yml) runs each language independently with `build-mode: none`, a 45-minute job timeout, and pinned actions maintained by Dependabot. Rust uses the repository's pinned toolchain. CodeQL can execute Rust build scripts and compile macros during extraction, but this workflow does not perform a full application build. It does not restore or populate dependency caches.

The [CodeQL configuration](../../../.github/codeql/codeql-config.yml) owns analysis exclusions: `test/` and `tests/` directories at any depth, `*.test.*` files, and Rust files named `tests.rs` or ending in `_tests.rs`. These locations contain test-only code; keep production implementations outside them. Application source, shipped starter templates, demo implementation, and CI workflows remain in scope. The default security queries remain enabled, including hard-coded credential detection; no severity or query is suppressed globally.

Path exclusions cannot remove individual inline `#[cfg(test)]` blocks from otherwise scanned source files. The flagged resource-state, SMTP-authentication, and webhook-delivery unit tests live in adjacent `tests/` directories and remain compiled through their original `#[cfg(test)]` modules. Their module names, assertions, and test execution are preserved. Other inline unit tests remain scanned; assess future alerts before moving a test module or excluding a file.

To activate this workflow when GitHub default setup is enabled:

1. Prepare the workflow and configuration changes for `main`. In repository **Settings → Advanced Security → CodeQL analysis**, choose **Switch to advanced** and disable default setup, following [GitHub's migration instructions](https://docs.github.com/en/code-security/how-tos/find-and-fix-code-vulnerabilities/configure-code-scanning/configuring-advanced-setup-for-code-scanning). Default setup otherwise blocks uploads from advanced workflows.
2. Merge the prepared changes and verify that the push-triggered CodeQL run completes for all three languages. You can also dispatch **CodeQL** manually against `main`. The weekly schedule starts only after the workflow exists on the default branch.
3. Inspect code-scanning tool status and alert locations to confirm production coverage and test exclusions. A local workflow lint does not prove successful extraction or uploaded results. If alerts persist only under a retired analysis configuration, inspect and remove that stale configuration using [GitHub's alert-resolution procedure](https://docs.github.com/en/code-security/how-tos/manage-security-alerts/manage-code-scanning-alerts/resolve-alerts#removing-stale-configurations-and-alerts-from-a-branch).

See [GitHub's configuration reference](https://docs.github.com/en/code-security/reference/code-scanning/workflow-configuration-options) for supported exclusion patterns. Workflow event filters only control when scanning starts; they do not exclude files from analysis.

## Artifacts and caches

Normal source checks share a development UI artifact only when its source/dependency inputs match. Linux native lint, tests, and process scenarios reuse one job's Cargo outputs. Provider and additional-platform jobs remain separate. Demo output has its own artifact and cannot satisfy the normal application build.

The CI entry point separates cache readers from writers. PR and non-main manual runs read caches; main push/manual runs can write. Nightly main runs can write, while branch runs read. Release workflows use `cache-mode: none`, so shipped artifacts and their validation do not consume mutable CI caches. Gates and deployment jobs also disable cache access.

The toolchain action respects the effective cache mode and fails on an unexpected value. Keep this policy aligned with `.github/scripts/cache-policy.test.mjs` when changing workflow calls or cache steps.

## Gates and deployment boundaries

The PR gate requires exactly one source-validation branch and a successful demo result. The demo result similarly requires exactly one reader/writer workflow branch. Selected jobs must complete successfully; failed, cancelled, missing, or unexpectedly skipped work cannot produce a successful gate.

Demo deployment depends on its own successful artifact, enabled configuration, and protected-environment approval. It can proceed independently of other source checks or the PR gate. Fork and Dependabot PRs receive build/test coverage without deployment credentials. See [demo deployment](demo-deployment.md).

Nightly and release do not deploy the demo. Releases have their own gate and publication credentials. See [releases](releases.md) for exact artifact and retry behavior. Required branch-protection settings are configured in GitHub; the workflow files alone do not prove those settings are active.

## Dependencies and workflow changes

Dependabot's checked-in configuration schedules updates. Maintainers must separately verify alerts/security-update settings in GitHub. Every PR, main, nightly and release source validation runs Rust advisory, license, source and bans checks. Release additionally runs root/browser npm audits and repeats Cargo advisory checks before optimized builds. Advisory checks detect known reported dependency vulnerabilities; they do not prove that all code is secure, and a new advisory requires a new check run to be detected.

CI shell commands use Node 26.8.1, including result and gate jobs. Workflow lint installs Go 1.26.10 explicitly and sets `GOTOOLCHAIN=local`, so a tool requiring a newer compiler fails instead of downloading a different Go release. Release jobs install Skopeo package `1.13.3+ds1-2ubuntu0.24.04.3` explicitly. Update these pins deliberately when upgrading tools.

Rust and its components are pinned in `rust-toolchain.toml`; Trunk, WASM tooling, the formatter, cargo-deny, actionlint, and the deployment CLI also have explicit versions. External GitHub Actions use commit SHAs and database images use digests. Hosted runner images and their system utilities still receive GitHub updates; these tool pins do not freeze the entire operating system.

Run these commands from the repository root with Node 26.8.1 and Go 1.26.10 installed:

```sh
node --test .github/scripts/*.test.mjs demo/tests/vercel-output.test.mjs tests/acceptance/init.test.mjs
GOTOOLCHAIN=local go run github.com/rhysd/actionlint/cmd/actionlint@v1.7.12 -shellcheck="" -pyflakes=""
```

Gate tests execute failure and skip handling; lint checks workflow syntax and expressions. Neither proves every path pattern selects the intended suite. For path-group changes, inspect representative documentation, UI, shared Rust, persistence, build, test-helper, and renamed/deleted paths with the pinned filter action. Run affected workflow stages when their behavior changes.

Record the actual hosted or local checks performed. Workflow definitions describe intended execution, not evidence of a successful run.
