---
title: Build and run locally
description: Prepare the pinned tools and run the server and administration interface from a development checkout.
sidebar:
  order: 100
---

This procedure builds the native server and its WebAssembly interface from source. It uses a separate local SQLite database and loopback listeners. To edit only documentation, use [the website workflow](../development-guides/documentation.md#preview-your-changes).

## Prepare your tools

Install Git, [rustup](https://rustup.rs/), and Node.js. Product CI uses Node **26.8.1**; website development uses **24.x**, as declared in `website/package.json`. Select the appropriate Node version before installing dependencies.

Native Rust compilation also needs a platform linker. On Windows, install Visual Studio Build Tools with **Desktop development with C++** and the Windows SDK. On macOS, install the Xcode command-line tools. On Linux, install your distribution's C/C++ build tools. Docker with Linux containers and Compose v2 is needed only for the [acceptance fixture](../testing/local-fixture.md) and server-database tests.

Clone your fork, or clone the upstream repository to inspect and build it:

```sh
git clone https://github.com/srothgan/ruphish.git
cd ruphish
```

Run the remaining commands from this directory, which contains `Cargo.toml` and `Trunk.toml`. `rust-toolchain.toml` selects the Rust toolchain, rustfmt, Clippy, and the `wasm32-unknown-unknown` target. Let rustup install these pinned components:

```sh
rustup show
node --version
npm --version
cargo install trunk --version 0.21.14 --locked --profile dev
npm ci --prefix crates/ruphish-ui/web
```

The browser dependencies supply editor assets and test utilities. Node is a build/test dependency; the native application does not need it at runtime.

## Build the interface

Set `NO_COLOR` before invoking Trunk. In PowerShell:

```powershell
$env:NO_COLOR = 'true'
```

In a POSIX shell:

```sh
export NO_COLOR=true
```

Then build:

```sh
trunk build --locked
```

Trunk generates the JavaScript/CSS inputs, compiles the Leptos interface to WASM, and publishes the complete distribution to `dist/admin/`. `dist/.stage/` is temporary staging. A successful build includes the manifest the server uses to verify its assets; do not edit or copy individual generated files.

## Create local configuration

Create a `data` directory if it does not exist. PowerShell:

```powershell
New-Item -ItemType Directory -Force data
```

POSIX shell:

```sh
mkdir -p data
```

Save the following as `data/development.json`. The directory is ignored by Git. Use a different filename and database path if you already have development data you want to preserve.

```json
{
  "admin_server": {
    "listen_url": "127.0.0.1:3333",
    "use_tls": false
  },
  "phish_server": {
    "listen_url": "127.0.0.1:8081",
    "use_tls": false
  },
  "db_name": "sqlite3",
  "db_path": "data/development.db",
  "logging": {
    "filename": "",
    "level": "info"
  }
}
```

This configuration uses HTTP on loopback for local development. With an explicit `--config` path, relative file paths resolve from the process working directory, not from the configuration file. Keep running from the repository root. Environment overrides still apply; check for previously configured overrides such as `DB_PATH` or `ADMIN_SERVER_LISTEN_URL` if the effective settings differ. See [configuration](../../administration/hosting-and-maintenance/configuration.md) for the complete contract.

## Start and sign in

```sh
cargo run -p ruphish --locked -- --config data/development.json
```

On the first successful initialization, the terminal displays the `admin` account's temporary password. Open **http://127.0.0.1:3333**, sign in, and set a new password when prompted. Restarting with the same database retains the account and does not print a new password. The participant listener is **http://127.0.0.1:8081**; it serves campaign pages after the necessary resources and campaign exist.

From a second terminal, check readiness:

```sh
curl --fail http://127.0.0.1:3333/health/ready
```

Use `curl.exe` in Windows PowerShell if `curl` resolves to a PowerShell alias. Expect HTTP `200` when the application is ready. Startup errors and application logs appear in the first terminal.

Use synthetic recipients and the [controlled acceptance fixture](../testing/local-fixture.md) when exercising delivery, domain verification, or participant interactions. An empty development database has no sending account; starting the server does not configure a mail service.

## Make a change and restart

Stop the server with **Ctrl+C**. For Rust server changes, rerun the same `cargo run` command. For UI, JavaScript, CSS, or browser dependency changes, rebuild with `trunk build --locked` before starting the server again. Refresh the browser after restart.

The ordinary development server loads `dist/admin/` from the checkout. `trunk serve` alone does not provide the server's authentication or API. The separate [public demo](../development-guides/demo.md) uses simulated responses. Release binaries embed the UI through the `embedded-assets` feature; see [releases](../maintenance/releases.md).

Stopping preserves `data/development.db`. To start a separate clean installation, use a new database filename in a separate configuration; do not delete data from an installation you still need. Never commit the data directory, credentials, or generated assets.

## Install tools for your change

Install only the additional tools required by your work:

| Work | Tool |
| --- | --- |
| Rust code | rustfmt and Clippy from the pinned Rust toolchain |
| Leptos markup | `leptosfmt` |
| WASM tests | `wasm-bindgen-cli`, which includes `wasm-bindgen-test-runner` |
| Maintained JavaScript | Root npm dependencies for Prettier and ESLint |
| Dependency policy | `cargo-deny` |

```sh
cargo install leptosfmt --version 0.1.33 --locked --profile dev
cargo install wasm-bindgen-cli --version 0.2.129 --locked --profile dev
npm ci --ignore-scripts
cargo install cargo-deny --version 0.19.9 --locked --profile dev
```

Tool versions come from workspace metadata, `Trunk.toml`, the lockfiles, and the checked-in CI toolchain action. `rust-analyzer.toml` integrates `leptosfmt`; put the installed formatter on your editor's PATH. Use Cargo/npm commands for dependency changes so manifests and lockfiles remain consistent.

Next, locate your change in [architecture](../development-guides/architecture.md) and choose [validation commands](../testing/validation.md). If setup fails, use [development troubleshooting](troubleshooting.md).
