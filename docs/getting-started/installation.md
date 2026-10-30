---
title: Install Ruphish
description: Choose a release binary or Docker image, configure your installation, and sign in for the first time.
sidebar:
  order: 4
---

<!-- Release documentation decision: Present Docker as the primary recommended deployment option. Publish native binaries as a secondary option for operators who manage their own hosting, service setup, and TLS certificates. Binary release scope covers building, packaging, and verification; it does not include service integration or installers. Update the installation guidance when distribution artifacts are published. -->

Docker is the primary recommended deployment option. Native release binaries are a secondary option for operators who manage their own hosting, services, and TLS certificates. Both provide the server and administration interface.

## Release binary

> TODO before publication: Publish the release binaries and document supported platforms, download links, integrity checks, generated configuration, and first startup. Verify the packaged administration interface and upgrade procedure. Release binaries are not available yet.

## Docker image

> TODO before publication: Publish the Docker image, replace the local image reference with its versioned registry reference, and verify startup, restart, and supported upgrades using the published image. The local image can be built now; registry distribution is not available yet. The acceptance fixture is a separate development tool.

The current image includes its administration UI and a default configuration at `/etc/ruphish/config.json`. It runs as UID/GID `10001:10001`, listens on HTTP ports `3333` (administration) and `8081` (public) inside the container, and writes application logs to stderr. Mount a persistent named volume at `/var/lib/ruphish` for the default SQLite database. Container replacement must reuse that volume; data written only into a container is lost when it is removed.

Mount a custom configuration read-only at `/etc/ruphish/config.json`, or select another file with `--config`. Additional flags such as `--disable-mailer` retain the image's configuration defaults. Bind the published administration port to host loopback for local access. Remote access needs trusted HTTPS through a reverse proxy or the application's TLS settings; mount the certificate chain and private key read-only at their configured paths and make the key readable by UID/GID `10001:10001`. See [listeners and TLS](../administration/listeners-and-tls.md) and [health probes](../administration/health.md) when changing listener ports or enabling TLS. The health-probe environment variables configure only the probe, not application listeners.

Local build and verification commands remain in the [acceptance fixture guide](../contributing/testing/local-fixture.md); they do not publish an image.

Once `ruphish:local` has been built, the repository's [compose.yaml](../../compose.yaml) starts a single application with a named SQLite volume and loopback-only published ports:

```sh
docker compose up -d --wait
docker compose logs ruphish
```

The initial password appears in container startup output unless supplied explicitly. Edit `environment` in the Compose file to override settings such as `ADMIN_SERVER_USE_TLS`, `LOGGING_LEVEL`, and `DB_NAME`; see the complete [environment reference](../administration/configuration.md#environment-overrides-and-docker-compose). Set Compose's `ADMIN_PORT` or `PHISH_PORT` variable to an unused host port if `3333` or `8081` is occupied or reserved. These two variables affect host port mappings only, not application listeners or the health probe. Run `docker compose down` to remove containers while retaining the named volume; do not add `--volumes` when you want to preserve data.

To connect the application to an existing PostgreSQL service, replace `DB_NAME` and add these environment entries:

```yaml
DB_NAME: "postgres"
POSTGRES_HOST: "postgres.example.internal"
POSTGRES_PORT: "5432"
POSTGRES_DATABASE: "ruphish"
POSTGRES_USERNAME: "ruphish"
POSTGRES_PASSWORD: "${DATABASE_PASSWORD:?Set DATABASE_PASSWORD}"
```

For MySQL, use `DB_NAME: "mysql"`, the `MYSQL_` equivalents, and port `3306`. Both providers verify TLS by default; mount your CA and set `POSTGRES_CA_PATH` or `MYSQL_CA_PATH` when using a private CA. Provision the database and account as described in [database administration](../administration/databases/index.md). This selects a database; it does not transfer existing SQLite data. Never configure both providers in the same application environment.

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

The generated settings bind administration to `https://127.0.0.1:3333` and the public listener to `http://127.0.0.1:8081`. SQLite, generated certificates and logs use absolute paths in the OS application directories, so restarting from a different working directory preserves the same installation. Builds without the `embedded-assets` feature load administration assets from `dist/admin` under the working directory.

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
