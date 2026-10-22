---
title: Install Ruphish
description: Choose a release binary or Docker image, configure your installation, and sign in for the first time.
sidebar:
  order: 4
---

The intended installation options are a release binary and a Docker image. Both will provide the server and administration interface.

## Release binary

> TODO before publication: Publish the release binaries and document supported platforms, download links, integrity checks, included configuration, and first startup. Verify the packaged administration interface and upgrade procedure. Release binaries are not available yet.

## Docker image

> TODO before publication: Publish the Docker image and document its registry, version tags, listener ports, persistent storage, configuration, and first startup. Verify startup, restart, and upgrades using the published image. The distribution image is not available yet; the local acceptance fixture is a separate development tool.

## Build from source

For development, the source-build instructions below remain available. Run commands from the product repository directory, `ruphish/`, which contains `Cargo.toml` and `config.example.json`. The configuration and startup commands below apply to this source-build path; packaged installation commands will be added above.

If you're following the [first-campaign tutorial](first-campaign.md), complete the prerequisites and build below, then go to the tutorial. It starts its own isolated installation; you don't need to start the native instance described later on this page.

## Prerequisites

- Rustup, with the toolchain selected by `rust-toolchain.toml`.
- Node.js and npm for building the browser assets. The repository's toolchain action pins Node 26.8.1; a different minimum version has not been established here.
- A native linker. On Windows, install the Visual Studio C++ build tools and Windows SDK. Linux and macOS need their usual native development tools.
- Network access for the initial dependency downloads.

The pinned Rust toolchain is 1.99.1 and includes the WASM target. These prerequisites apply to building from source.

## Build

```sh
rustup show
cargo install trunk --version 0.21.14 --locked --profile dev
cargo fetch --locked
npm ci --prefix crates/ruphish-ui/web
trunk build --locked
cargo build -p ruphish --locked -j 1
```

If your environment sets `NO_COLOR=1`, Trunk rejects that value. Before building, set `$env:NO_COLOR = 'true'` in PowerShell or `export NO_COLOR=true` in a POSIX shell.

Trunk writes the completed administration assets to `dist/admin/`. Keep that distribution with this checkout; the server checks its manifest at startup. Node/npm are build tools, not server runtime dependencies.

## Configure a fresh local instance

Copy the example only when `config.json` does not already exist. PowerShell:

```powershell
$env:RUPHISH_CONFIG = './config.json'
if (-not (Test-Path -LiteralPath $env:RUPHISH_CONFIG)) {
    Copy-Item -LiteralPath config.example.json -Destination $env:RUPHISH_CONFIG
}
$env:GOPHISH_INITIAL_ADMIN_PASSWORD = Read-Host 'Temporary administrator password' -MaskInput
cargo run -p ruphish --locked -- --config $env:RUPHISH_CONFIG --mode all
```

The masked prompt requires PowerShell 7. `RUPHISH_CONFIG` is an example-shell variable passed through `--config`, not an automatic configuration override. In a POSIX shell, prepare the file with:

```sh
export RUPHISH_CONFIG='./config.json'
test -e "$RUPHISH_CONFIG" || cp config.example.json "$RUPHISH_CONFIG"
```

Then export `GOPHISH_INITIAL_ADMIN_PASSWORD` using your preferred secret-input mechanism before starting the application:

```sh
cargo run -p ruphish --locked -- --config "$RUPHISH_CONFIG" --mode all
```

Choose your own temporary password; do not reuse tutorial credentials.

The example binds the administration listener to `https://127.0.0.1:3333` and the public listener to `http://127.0.0.1:8081`. Its SQLite file is `ruphish.db`. Relative paths are resolved from the process working directory, even when the configuration file is elsewhere.

The public HTTP example is enough to start the application, but campaign origins require verified HTTPS. Use the local fixture for the tutorial, or configure [listeners and TLS](../administration/listeners-and-tls.md) for your domain.

## Sign in

Wait for the `RUPHISH_READY` message on stderr, then open the administration address. For locally generated development certificates, inspect the certificate before accepting a browser exception. Do not use that development trust arrangement as your public TLS setup.

Sign in as `admin` using the temporary password you supplied. Complete the required password change. The application does not print passwords or API tokens. If the first startup generated an inaccessible temporary password, stop it and restart with `GOPHISH_INITIAL_ADMIN_PASSWORD` set.

After stopping the process, remove the temporary variable from your shell with `Remove-Item Env:GOPHISH_INITIAL_ADMIN_PASSWORD` in PowerShell or `unset GOPHISH_INITIAL_ADMIN_PASSWORD` in a POSIX shell. An established account password is preserved on subsequent starts.

## Stop and restart

Press Ctrl+C for orderly shutdown. Start again from the same working directory with the same configuration. Keep the database, configuration, certificate keys, and assets intact.

For an existing database, read [migration](../migration/index.md) before startup. Do not test an upgrade against your only copy.

## If startup fails

| Symptom | Check and correction |
| --- | --- |
| Missing or stale administration assets | Run `trunk build --locked` from the product root and retry |
| Address already in use | Stop the conflicting local process or select a free listener address in your configuration |
| Configuration rejected | Compare field names/types with the [configuration reference](../administration/configuration.md); unknown fields are rejected |
| Certificate/key error | Check both configured paths and the matching pair; do not replace only one file |
| Database schema rejected | Stop and follow [migration recovery](../migration/verification-and-rollback.md); do not initialize over existing data |

Next, [run a local campaign](first-campaign.md).
