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
| `release.yml` | Version tag push or manual dispatch | Full source checks, dependency security, optimized builds, and artifact verification; only matching tag pushes publish |

CI starts even for documentation and housekeeping changes so its final gate can report a result. A documentation-only change does not select product acceptance unless it also changes a broader input. Website checks and deployment follow the website's separate Vercel configuration.

## Path-selected checks

| Group | Main inputs | Checks |
| --- | --- | --- |
| Product | Crates, product tests, demo data, runtime image files, shared build inputs | Browser checks/build, native Clippy/tests and executable workflows, Windows x64 and macOS ARM64 acceptance |
| Persistence | Domain/application/adapter crates, tests, migrations, shared build inputs | Both supported MySQL and PostgreSQL versions, histories, schema recovery, and provider executable scenarios |
| Public | Acceptance fixture, runtime configuration and Docker inputs, shared build inputs | Packaged image and controlled DNS/HTTPS/SMTP/privacy scenarios |
| Dependencies | Shared build inputs | Rust license, source, and banned-dependency policy |
| Demo | UI/domain/contracts, starter content, demo data, build/deployment inputs | Demo contracts, browser checks, optimized build, and static deployment artifact |

Shared build inputs include manifests, lockfile, toolchain, formatter/dependency policy, Trunk configuration, workflow/action implementations, and release scripts. The YAML file is authoritative. Add new source/build locations there when required; do not construct commands dynamically from changed filenames.

The always-run quality job installs root npm tools, checks formatting/lint, runs workflow/fixture-initialization script tests, and lints workflows. Changes limited to script test files still exercise these checks without selecting product builds. Issue templates, CODEOWNERS, and Dependabot configuration do not select product acceptance by themselves.

The pinned path-filter action compares PR merge commits with their base using Git. Push selection uses changes since the prior main revision. Full checkout history supports deleted paths and both sides of renames. Nightly, release, and manual full runs bypass filtering.

## Artifacts and caches

Normal source checks share a development UI artifact only when its source/dependency inputs match. Linux native lint, tests, and process scenarios reuse one job's Cargo outputs. Provider and additional-platform jobs remain separate. Demo output has its own artifact and cannot satisfy the normal application build.

The CI entry point separates cache readers from writers. PR and non-main manual runs read caches; main push/manual runs can write. Nightly main runs can write, while branch runs read. Release workflows use `cache-mode: none`, so shipped artifacts and their validation do not consume mutable CI caches. Gates and deployment jobs also disable cache access.

The toolchain action respects the effective cache mode and fails on an unexpected value. Keep this policy aligned with `.github/scripts/cache-policy.test.mjs` when changing workflow calls or cache steps.

## Gates and deployment boundaries

The PR gate requires exactly one source-validation branch and a successful demo result. The demo result similarly requires exactly one reader/writer workflow branch. Selected jobs must complete successfully; failed, cancelled, missing, or unexpectedly skipped work cannot produce a successful gate.

Demo deployment depends on its own successful artifact, enabled configuration, and protected-environment approval. It can proceed independently of other source checks or the PR gate. Fork and Dependabot PRs receive build/test coverage without deployment credentials. See [demo deployment](demo-deployment.md).

Nightly and release do not deploy the demo. Releases have their own gate and publication credentials. See [releases](releases.md) for exact artifact and retry behavior. Required branch-protection settings are configured in GitHub; the workflow files alone do not prove those settings are active.

## Dependencies and workflow changes

Dependabot's checked-in configuration schedules updates. Maintainers must separately verify alerts/security-update settings in GitHub. PR/nightly source checks retain dependency license/source/bans policy; release additionally runs root/browser npm audits and Cargo advisory checks before optimized builds.

Run these commands from the repository root with the product Node version and Go installed for pinned actionlint:

```sh
node --test .github/scripts/*.test.mjs demo/tests/vercel-output.test.mjs tests/acceptance/init.test.mjs
go run github.com/rhysd/actionlint/cmd/actionlint@v1.7.12 -shellcheck="" -pyflakes=""
```

Gate tests execute failure and skip handling; lint checks workflow syntax and expressions. Neither proves every path pattern selects the intended suite. For path-group changes, inspect representative documentation, UI, shared Rust, persistence, build, test-helper, and renamed/deleted paths with the pinned filter action. Run affected workflow stages when their behavior changes.

Record the actual hosted or local checks performed. Workflow definitions describe intended execution, not evidence of a successful run.
