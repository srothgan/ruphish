---
title: Install Ruphish
description: Choose a release binary or Docker image, configure your installation, and sign in for the first time.
sidebar:
  order: 4
---

The intended installation options are a release binary and a Docker image. Both will provide the server and administration interface.

## Release binary

> TODO before publication: Publish the release binaries and document supported platforms, download links, integrity checks, generated configuration, and first startup. Verify the packaged administration interface and upgrade procedure. Release binaries are not available yet.

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

Run from the product root after building the administration assets:

```sh
cargo run -p ruphish --locked
```

On first startup, Ruphish creates `config.json` in the [OS configuration directory](../administration/configuration.md#default-locations-and-explicit-configuration), selects SQLite, and creates the required data and log directories. Later starts read that file without rewriting it. Startup prints the selected paths. Edit the file and restart to change listeners, logging, or the database provider.

The generated settings bind administration to `https://127.0.0.1:3333` and the public listener to `http://127.0.0.1:8081`. SQLite, generated certificates and logs use absolute paths in the OS application directories, so restarting from a different working directory preserves the same installation. Development builds still load administration assets from `dist/admin` under the working directory until release asset embedding is implemented.

For an existing local configuration or an explicitly managed installation, pass its path:

```sh
cargo run -p ruphish --locked -- --config ./config.json
```

That file must exist. Relative paths inside an explicitly selected configuration retain their working-directory meaning. This mode does not create a per-user installation. The [configuration reference](../administration/configuration.md) provides a copyable JSON example; downloading a separate example file is unnecessary.

The public HTTP example is enough to start the application, but campaign origins require HTTPS and an enabled owned domain. Verification is additionally required when enabled in [company settings](../administration/settings.md). Use the local fixture for the tutorial, or configure [listeners and TLS](../administration/listeners-and-tls.md) for your domain.

## Sign in

For automated monitoring, use the [liveness and readiness probes](../administration/health.md). They are available on each enabled listener after HTTP startup, without administrator credentials.

Wait for the `RUPHISH_READY` message on stderr, then open the administration address. For locally generated development certificates, inspect the certificate before accepting a browser exception. Do not use that development trust arrangement as your public TLS setup.

Sign in as `admin` using the temporary password printed to startup stderr. Alternatively, supply your own temporary password through `GOPHISH_INITIAL_ADMIN_PASSWORD` before starting. Generated password output works for both terminal and noninteractive launches, including Docker. Complete the required password change. Generated passwords bypass the application log file, but container logs or redirected stderr can retain this deliberate startup output. Supplied passwords and API tokens are never printed.

If you supplied a temporary password, remove the temporary variable from your shell after stopping the process, using `Remove-Item Env:GOPHISH_INITIAL_ADMIN_PASSWORD` in PowerShell or `unset GOPHISH_INITIAL_ADMIN_PASSWORD` in a POSIX shell. An established account password is preserved on subsequent starts.

## Stop and restart

Press Ctrl+C for orderly shutdown. Start again using the same OS account or explicit configuration. Source builds still need their administration assets in the working directory; explicit configurations also retain working-directory-relative paths. Keep the database, configuration, certificate keys, and assets intact.

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
