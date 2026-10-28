---
title: Set up a development checkout
description: Install the pinned tools, build the native server and WASM UI, and run focused checks.
sidebar:
  order: 2
---

Run commands from the product root, the directory containing `Cargo.toml`. Follow [installation](../getting-started/installation.md) to build the application, configure local listeners, and establish the administrator account. Node/npm are browser build tools; the native server does not need Node at runtime.

## Use the pinned tools

`rust-toolchain.toml` owns the Rust version, components, and WASM target. Workspace metadata in `Cargo.toml` owns developer-tool versions, and `Trunk.toml` owns Trunk configuration. The lockfiles own dependency resolution. Use Cargo/rustup commands for dependency and tool changes rather than hand-editing version entries.

The [validation prerequisites](testing/validation.md#prerequisites) contain exact installation commands. Install only the tools needed for your work. On Windows, native compilation requires the Visual Studio C++ build tools and Windows SDK; other hosts need a working native Rust linker.

## Editor and formatting

Open the product directory as the editor workspace. `rust-analyzer.toml` configures `leptosfmt --stdin --rustfmt`; ensure the pinned Leptos formatter is on the editor's PATH. Rustfmt and Leptos formatting use the checked-in configurations, four-space indentation, LF line endings, and a 100-column target for Rust/markup.

After applicable edits, format and check:

```sh
leptosfmt crates/ruphish-ui/src crates/ruphish-ui/tests
cargo fmt --all
leptosfmt --check crates/ruphish-ui/src crates/ruphish-ui/tests
cargo fmt --all -- --check
```

Markdown follows a separate rule: one source line per paragraph or list item, with no manual prose wrapping.

Maintained JavaScript uses the root `package.json` and lockfile for pinned Prettier and ESLint tools. The root configurations support editor integration, two-space indentation, single quotes, LF line endings and a 100-column formatting target. Generated bundles, dependencies and independent content fixtures are excluded. These tools are separate from the browser build dependencies in `crates/ruphish-ui/web/`; see [JavaScript formatting and linting](testing/validation.md#javascript-formatting-and-linting) for the direct installation, check and formatting commands.

## Build and run

`trunk build --locked` builds the WASM interface and its generated browser assets. Then build/run the native `ruphish` package with the prepared configuration. Use the native admin listener to exercise authentication and API-backed pages; standalone `trunk serve` is not configured as a replacement for that application flow.

Trunk stages output in `dist/.stage/` and publishes the complete browser distribution to `dist/admin/`. The server checks its manifest before readiness. Rebuild when its inputs change; stale assets cannot validate changed UI source. See [frontend development](frontend.md).

Keep heavy local work sequential. In PowerShell, set `NO_COLOR` to the literal string `true` before Trunk; `1` is not accepted as a boolean by the pinned version. The validation guide provides bounded Cargo/test concurrency settings.

## Make and verify a focused change

Start with a regression that demonstrates the changed guarantee. Choose its authoritative owner using [architecture](architecture.md), then check its consumers. Use [validation](testing/validation.md) for the direct commands appropriate to the change. Do not run complete provider acceptance merely because you changed a paragraph or a local pure function.

Generated output, `target/`, `dist/`, local fixture state, and secrets are not source contributions. Review your diff before presenting the change, and report tests actually executed rather than treating a clean compile as workflow acceptance.
