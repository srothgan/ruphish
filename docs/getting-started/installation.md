---
title: Install Ruphish
description: Choose a release binary or Docker image, configure your installation, and sign in for the first time.
sidebar:
  order: 4
---

Docker is the recommended option for persistent server hosting. Native binaries provide a local tryout without a Rust or Node installation. Both include the server and administration interface. The native installer does not create a service or start Ruphish automatically.

## Release binary

The installer sources and website release-selection API are in this repository. The commands below describe their intended deployed URLs; website deployment and verification of those URLs are still required before announcing installer availability. See [GitHub Releases](https://github.com/srothgan/ruphish/releases) for published assets; an in-progress workflow or draft is not an installable release.

On Linux or macOS, run:

```sh
curl -fsSL https://www.ruphish.com/install.sh | sh
```

In Windows PowerShell 5.1 or PowerShell 7, run:

```powershell
irm https://www.ruphish.com/install.ps1 | iex
```

The default selects the highest published full version. Before a full release exists, it reports that none is available. To include development previews alongside full releases:

```sh
curl -fsSL https://www.ruphish.com/install.sh | sh -s -- --include-previews
```

```powershell
& ([scriptblock]::Create((irm https://www.ruphish.com/install.ps1))) -IncludePreviews
```

Previews are for disposable installations; upgrades between previews or into a final release are not guaranteed. Selection follows version precedence, so `0.1.0-dev.2` precedes `0.1.0-dev.10`, which precedes `0.1.0`. The preview option can also select a full release. Each invocation resolves a version once and downloads its archive and `SHA256SUMS` from the same exact GitHub release tag.

The installer targets Linux x64/ARM64 with glibc 2.36 or newer, macOS 15 or newer on Apple Silicon, and Windows x64/ARM64. Linux/macOS require `curl`, `tar`, standard POSIX utilities, and `sha256sum` or `shasum`. Windows uses built-in .NET download, ZIP, and hashing support. The staged executable must successfully report its version before replacement; missing runtime libraries or an unsupported OS leave the current installation intact. Native archives are unsigned, and macOS archives are not notarized. The installer does not disable operating-system protections.

Program files live in `~/.local/share/ruphish-install/current/` with a command link at `~/.local/bin/ruphish`, or `%LOCALAPPDATA%\Programs\Ruphish\current\` on Windows. User PATH setup is optional and never changes the machine PATH. Open a new terminal after installation, or use the full-path launch command printed by the installer. A subprocess cannot update its parent shell's environment.

### Inspect, choose a version, or change the destination

Download the script first if you want to inspect it or pass several options:

```sh
curl -fsSL https://www.ruphish.com/install.sh -o install.sh
less install.sh
sh install.sh --help
sh install.sh --version 0.1.0-dev.1 --no-modify-path
```

```powershell
Invoke-WebRequest https://www.ruphish.com/install.ps1 -OutFile install.ps1
Get-Content .\install.ps1
& .\install.ps1 -Help
& .\install.ps1 -Version 0.1.0-dev.1 -NoModifyPath
```

These version examples require that exact release to have been published. `--version` / `-Version` bypasses website version discovery and can select a preview without the preview flag. `--install-dir` / `-InstallDir` selects a dedicated absolute program directory; use the same option on future updates and removal. Existing unmanaged directories and application-data locations are refused. `--no-modify-path` / `-NoModifyPath` leaves persistent PATH configuration untouched. Do not append arguments to `irm | iex`; use the script-block form above or a saved script.

### Update or remove a native installation

Stop Ruphish, then rerun the installer with the desired selection options. Repeating the installed version does not replace its program files. Preview opt-in applies to each invocation. Installation never changes application configuration, databases, certificates, or logs; starting the new executable may perform migrations, so follow [backup and recovery](../administration/backup-and-recovery.md) before using existing data.

A lower version triggers a prominent downgrade warning. Interactive terminals offer `Proceed? [y/N]`, with Stop as the default. Unattended runs fail unless `--allow-downgrade` / `-AllowDowngrade` is explicitly supplied. Proceeding replaces program files only: it does not reverse migrations or restore configuration. Prefer restoring a compatible backup or using separate disposable application data when returning to an older version.

Download, checksum, archive-layout, or staged-executable failures preserve the installed program. An interrupted directory replacement is recovered on the next run. On Windows, stop processes holding the executable before retrying. If command-access setup fails after replacement, the installer has already printed the resulting program path; use that path while correcting PATH configuration.

To remove the managed installation, run the saved script with `sh install.sh --uninstall` or `& .\install.ps1 -Uninstall`. Include a custom installation directory if you used one. Removal deletes installer-owned program files, command links, and PATH entries it owns. Application data is retained; see [default locations](../administration/configuration.md#default-locations-and-explicit-configuration). There is no automatic data-purge option.

Start an installed native binary with `ruphish`, then follow [first startup](#configure-a-fresh-local-instance) below. No configuration or data is created until you launch the application.

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

For an installed native binary, run `ruphish`. For a source build, run from the product root after building the administration assets:

```sh
cargo run -p ruphish --locked
```

On first startup, Ruphish creates `config.json` in the [OS configuration directory](../administration/configuration.md#default-locations-and-explicit-configuration), selects SQLite, and creates the required data and log directories. Later starts read that file without rewriting it. Startup prints the selected paths. Edit the file and restart to change listeners, logging, or the database provider.

The generated settings bind administration to `https://127.0.0.1:3333` and the public listener to `http://127.0.0.1:8081`. SQLite, generated certificates and logs use absolute paths in the OS application directories, so restarting from a different working directory preserves the same installation. Builds without the `embedded-assets` feature load administration assets from `dist/admin` under the working directory.

For an existing local configuration or an explicitly managed installation, pass its path:

```sh
cargo run -p ruphish --locked -- --config ./config.json
```

For an installed binary, the equivalent command is `ruphish --config ./config.json`. That file must exist. Relative paths inside an explicitly selected configuration retain their working-directory meaning. This mode does not create a per-user installation. The [configuration reference](../administration/configuration.md) provides a copyable JSON example; downloading a separate example file is unnecessary.

The public HTTP example can serve campaigns using a registered, enabled HTTP origin when verification is off. If verification is required in [company settings](../administration/settings.md), only DNS hostnames over trusted HTTPS are eligible. Use the local fixture for the tutorial, or configure [listeners and TLS](../administration/listeners-and-tls.md) for your domain.

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
