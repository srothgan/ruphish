---
title: Fuzz input boundaries
description: Run bounded fuzz targets, reproduce failures, and turn discoveries into ordinary regression tests.
sidebar:
  order: 315
---

Fuzzing explores combinations of input that example tests may miss. The isolated [fuzz package](../../../fuzz/Cargo.toml) calls production decoding, routing, importing, and rendering code with synthetic data. Its assertions check selected contracts in addition to crashes. It does not connect to databases, mailboxes, or external recipients, and does not start an application server.

## Choose a target

| Target | Boundary exercised |
| --- | --- |
| `submission_decode` | Closed JSON submission decoding, including malformed and unexpected fields |
| `public_routing` | Real in-memory public HTTP router with recording application services |
| `mailbox_identify` | MIME decoding, sender restrictions, nested messages, and recipient/origin correlation |
| `template_render` | Go-template execution, output limits, and error recovery |
| `csv_import` | CSV decoding, column mapping, row isolation, and recipient results |
| `page_bundle_raw` | Malformed ZIP bytes passed through the page-bundle import boundary |
| `page_bundle_structured` | Generated ZIP containers that reach HTML, CSS, and asset resolution |
| `email_html` | Email content policy and managed tracking images |
| `landing_html` | Distinct authored landing-page and static learning-page policies |

Routing doubles record the actual arguments dispatched by Axum; they do not establish persistence, authorization inside a provider, or submission idempotency. Template rendering preserves text-template semantics rather than automatically escaping HTML. Authored landing scripts are intentionally supported, whereas static learning pages and emails have different content restrictions. Keep expectations tied to the target's contract rather than applying one sanitizer rule everywhere.

The harness library has ordinary tests for curated cases and assertion behavior. Run these from the repository root using the product toolchain:

```sh
cargo +1.99.1 test --manifest-path fuzz/Cargo.toml --lib --locked
```

This checks the harness and its deterministic cases; it does not perform coverage-guided exploration or enable a sanitizer. Existing product regressions remain the place for persisted effects, recovery, and protocol guarantees outside these boundaries.

## Prepare fuzzing tools

Linux CI uses the nightly selected by [fuzz/rust-toolchain.toml](../../../fuzz/rust-toolchain.toml) and cargo-fuzz **0.13.2**. The normal product toolchain is unchanged. Install a C/C++ compiler and linker, then run:

```sh
cargo +1.99.1 install cargo-fuzz --version 0.13.2 --locked --profile dev
cd fuzz
rustup show
cargo +1.99.1 fetch --locked
cargo fuzz list
```

The following fuzz commands run from `fuzz/`, so rustup selects the separate pinned nightly. Cargo-fuzz uses an optimized build with AddressSanitizer by default; this materially improves exploration throughput over `--dev` while retaining the target assertions and sanitizer checks. Use Linux for coverage-guided campaigns. Ordinary harness tests run on Windows, and upstream documents [native Windows fuzzing setup](https://rust-fuzz.github.io/book/cargo-fuzz/windows/setup.html), but local validation with this pinned toolchain did not produce a runnable Windows fuzz binary: the AddressSanitizer build lacked `clang_rt.asan_dynamic_runtime_thunk-x86_64.lib`, and disabling the sanitizer still left unresolved coverage-counter symbols. Ordinary harness-test success therefore does not establish Windows fuzzing support. CI keeps AddressSanitizer enabled on Linux.

Cargo-fuzz 0.13.2 has no `--locked` build option. The ordinary harness tests also compare the separate manifest's patch declarations and shared locked package identities against the product lockfile; they do not establish identical resolved feature graphs. Fetch the checked-in resolution first, keep subsequent fuzz builds offline, and inspect lockfile changes. In PowerShell:

```powershell
$env:CARGO_NET_OFFLINE = 'true'
```

In a POSIX shell:

```sh
export CARGO_NET_OFFLINE=true
```

Unset this variable when finished if later dependency work needs the network. The separate manifest must retain the product's patched dependencies, including SQLx and the temporary `gtmpl-ng` escape-decoder fix. Update dependencies through Cargo, review both lockfiles and patch declarations, and rerun the relevant harness tests after changing them. The template seed `utf8-escape-crash.txt` preserves the malformed Unicode escape found by scheduled fuzzing; the application template regressions also verify typed rejection and subsequent rendering.

## Run a bounded campaign

Keep curated inputs in `seeds/<target>/` and evolving inputs in the ignored `corpus/<target>/`. The first corpus directory receives new discoveries; the seed directory remains unchanged. Create the writable directory for your chosen target, then run:

```sh
cargo fuzz run csv_import corpus/csv_import seeds/csv_import -- -max_total_time=60 -timeout=5 -rss_limit_mb=2048 -max_len=65536
```

Create `corpus/csv_import` with `mkdir -p corpus/csv_import` on a POSIX shell, or `New-Item -ItemType Directory -Force corpus/csv_import` in PowerShell. Replace the target and both directory names together when selecting another boundary. The [workflow](../../../.github/workflows/fuzz.yml) records target-specific input limits.

`-max_total_time` bounds exploration time after compilation; `-timeout` bounds one input, `-rss_limit_mb` limits process memory, and `-max_len` limits generated input size. A timeout or memory-limit failure is a finding to reproduce and diagnose. Expected input-validation errors are normal outcomes. Precise multi-megabyte boundary cases belong in deterministic tests rather than every iteration of a short campaign.

When done, return to the repository root and check that the build did not alter dependency resolution:

```sh
git diff --exit-code -- Cargo.lock fuzz/Cargo.lock
```

An empty diff establishes unchanged lockfiles, not sufficient behavioral coverage. Record the toolchain, target, command, elapsed budget, execution count, and any failures. A short clean run establishes only what that run exercised.

## Reproduce and retain failures

Cargo-fuzz writes a triggering input beneath `artifacts/<target>/` and prints its path. From `fuzz/`, replay that exact file with the same build mode and limits:

```sh
cargo fuzz run csv_import artifacts/csv_import/crash-INPUT_HASH -- -timeout=5 -rss_limit_mb=2048
cargo fuzz tmin csv_import artifacts/csv_import/crash-INPUT_HASH -- -timeout=5 -rss_limit_mb=2048
```

Replace `crash-INPUT_HASH` with the actual artifact filename; timeout and memory failures may use different prefixes. Diagnose whether the failure is in production code, an invalid harness expectation, or the environment. Do not hide a production failure by narrowing generators or catching panics. Minimize the reproduction, fix the owning implementation, and add an ordinary regression asserting the intended result. Retain a small synthetic seed when it improves future exploration; do not commit the entire generated corpus or real participant data.

Replay a target's curated seeds without a mutation campaign using:

```sh
cargo fuzz run csv_import seeds/csv_import -- -runs=0 -timeout=5 -rss_limit_mb=2048
```

When changing a harness, keep an independently known success and rejection case, and check that its assertion would fail for a deliberately incorrect result. Random bytes alone often stop at syntax rejection; structured cases should reach the transformation whose guarantees are under test. Do not use the production validator itself as the expected answer.

## CI behavior

The [fuzz workflow](../../../.github/workflows/fuzz.yml) selects fixed contract, HTTP, and adapter target groups through [.github/paths.yml](../../../.github/paths.yml). Pull requests receive 30 seconds of exploration per selected target after compilation and deterministic harness tests. Scheduled Tuesday runs at 05:19 UTC and manual dispatches select all targets with 300 seconds per target. Jobs have a 90-minute wall-clock limit, including setup and builds.

Pull requests and manual runs on other branches can restore caches but cannot populate them. Scheduled or manual runs on the default branch can save successful exploration corpora. Generated corpora and available reproductions are uploaded with 14-day retention, including on failures. Checked-in seeds remain separate from restored corpus data. Workflow failures remain visible; no fuzz exit status is discarded.

These are independent workflow checks. Their presence does not configure GitHub branch-protection requirements. The schedule becomes active after the workflow reaches the default branch; a workflow definition is not evidence that a hosted campaign has run.
