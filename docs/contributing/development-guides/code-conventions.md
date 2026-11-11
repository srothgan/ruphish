---
title: Code conventions
description: Apply the shared rules for implementation, Rust safety, state, dependencies, and validation.
sidebar:
  order: 215
---

These conventions apply across the codebase. Use the [architecture](architecture.md) to locate the implementation boundary, then the relevant backend, database, or frontend guide for detailed procedures. The [root contribution guide](../../../CONTRIBUTING.md) explains proposing and submitting work.

## Make a coherent change

Inspect the implementation, consumers, tests, and documentation before changing behavior. Follow both success and failure paths, including recovery where state or external effects are involved. Check installed dependency APIs rather than assuming support from a different version.

Extend existing helpers, components, and constants at the place that owns the behavior. Share implementations when they represent the same responsibility and are expected to change together. Similar-looking code alone does not justify an abstraction. Avoid adding a local copy of a rule or a compensating check when the original implementation can be corrected.

Keep the solution focused on the current problem. Add abstractions, caching, configuration, or concurrency only when they solve a concrete need. Do not introduce placeholders or no-op implementations that report success.

Before adding caching or concurrency, look for repeated queries, requests, computations, or transformations that can be eliminated, safely reused, or batched. Preserve authorization, data freshness, and operation ordering when sharing results. Introduce more complex optimization only when there is evidence of a performance need.

When changing a contract, update its types, callers, persistence, tests, and documentation together. Explain breaking changes and their migration consequences. Keep compatibility code only when it has an explicit purpose; remove superseded paths, unused dependencies, obsolete flags, and stale comments within the changed scope.

Remove a repeated check only after establishing which type, earlier validation, or invariant guarantees the condition and whether that guarantee still holds where the check runs. Preserve validation at trust boundaries, authorization checks, and checks needed after concurrent state changes. A similar check elsewhere does not by itself make one redundant.

## Organize modules by responsibility

Group related files under the feature or responsibility that owns them. Use the containing folder as context instead of repeating it in every filename, such as `campaigns/queries.rs`. Keep independent modules as individual files; do not create single-file folders solely for symmetry. Use `mod.rs` as the entry point for grouped modules, preserve visibility at the owning boundary, and update imports, embedded assets, tests, build inputs, and documentation when moving files. Operational product checks belong in diagnostics modules, while test modules contain automated regressions.

## Model state and outcomes explicitly

Give mutable state one owner and derive dependent values. Represent mutually exclusive states directly rather than maintaining several booleans that can contradict each other. Separate an editable draft from confirmed data, and define how necessary caches become invalid.

Use typed outcomes for expected success and failure. Propagate errors with useful context while keeping secrets and participant input out of diagnostics. A fallback must not turn an unsuccessful operation into apparent success.

For asynchronous work, prevent stale responses and duplicate effects. Handle cancellation, retry, and uncertain outcomes where they can occur. Release tasks, listeners, subscriptions, and other resources when their owner ends. See [frontend state](frontend-development.md#give-each-state-one-owner) and [worker development](backend-development.md#change-a-worker).

## Preserve Rust safety

The workspace forbids unsafe code and denies ignored must-use results. Production code must avoid accidental `unwrap`, `expect`, `panic`, `exit`, `todo`, `unimplemented`, and `dbg` usage. Handle expected failures through the existing result/error types instead of bypassing them.

Keep test-only Clippy exemptions limited to assertions and fixture setup that must fail the test. Do not broaden a lint exemption across production code to make a check pass. Workspace lint configuration in `Cargo.toml` is the executable authority; crate manifests inherit it.

## Keep tooling reproducible

Use the [pinned toolchain](../development-setup/index.md) and checked-in lockfiles. Let Cargo/rustup and npm update dependency/tool inputs rather than manually inventing lockfile entries. Keep dependency exceptions narrow, and retain attribution and license notices for reused material.

### Dependency license policy

In `deny.toml`, license exceptions pair a crate name with explicitly reviewed licenses across versions. For example, `crate = "lol_html"` with `allow = ["BSD-3-Clause"]` permits that license for updates of `lol_html` without permitting it for unrelated crates. Keep one exception per crate; restrict its version only when approval depends on release-specific licensing, and document that reason beside the exception.

Cargo-deny evaluates SPDX license expressions against the global allowlist and matching crate exceptions. A change to an unapproved license fails unless the expression offers an approved alternative; a change to a globally approved license can pass. Do not broaden the global allowlist to silence a dependency update. Review licensing changes and keep [third-party notices](../../../THIRD-PARTY-NOTICES.md) accurate for the versions actually distributed. License exceptions do not bypass advisory, yanked-package or source checks.

Run `cargo deny check bans licenses sources` after policy changes. Run the separate advisory check required by [release validation](../maintenance/releases.md#what-the-workflow-verifies) before publication.

### Commands and generated assets

Run documented commands directly. Do not add test-runner wrappers, Cargo aliases, or npm wrappers around existing checks. CI uses fixed commands and shared path groups; see [CI workflows](../maintenance/ci-workflows.md).

Run `npm ci` for a fresh dependency installation or changed dependency inputs. Reuse generated assets only when their inputs match. Let Trunk generate its inputs and publish through `dist/.stage/` to `dist/admin/`; do not edit generated output or weaken manifest checks. Use development builds for ordinary work and optimized builds for release validation.

## Review and verify

Use focused regressions while implementing, then run the applicable [validation commands](../testing/validation-commands.md) after edits settle. Diagnose failures before rerunning them. Repeating an unchanged successful check or compiling an identical target twice does not add evidence.

Review the final diff and affected consumers for correctness, unrelated edits, obsolete code, secrets, and mismatches between implementation and documentation. Report the exact commands and outcomes, including unavailable or unexecuted checks. Keep unrelated findings separate from the change rather than expanding its scope silently.

For each confirmed review finding, identify the file and line, explain the evidence and practical impact, and recommend the smallest safe fix at the source of the problem. Prioritize incorrect behavior, security violations, divergent state, and substantial maintenance costs. Distinguish an unverified risk from a confirmed defect, and state any missing verification. If no material issues are found, say so; do not invent findings to fill a checklist.
