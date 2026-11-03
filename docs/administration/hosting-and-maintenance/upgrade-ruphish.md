---
title: Upgrade Ruphish
description: Replace the released application, let startup apply supported schema updates, and verify the installation.
sidebar:
  order: 33
---

An upgrade replaces the Ruphish binary or Docker image while keeping the existing database and configuration. Supported database changes run automatically at startup; there is no separate migration command or directory to copy.

For an existing Gophish installation, use [Migration from Gophish](../../migration/migrate-the-database.md) instead. Changing from SQLite to a server database is also a separate data-movement task; changing `db_name` does not perform it.

## Prepare

1. Read the target release's notes for compatibility requirements and configuration changes.
2. Record the current version with `ruphish --version`, or inspect the current Docker image with `docker compose images ruphish`. Keep the old executable or image available for recovery.
3. Plan a maintenance window. Ruphish and campaign links are unavailable while the service is stopped.
4. Stop every application process using the database, including separate administration or campaign processes.
5. Take and check a [backup](back-up-and-restore.md) of the database, configuration, and required secrets.

Do not run old and new versions against the same database during the upgrade. For native binaries, the [installer reference](native-installer-reference.md) explains how to select an exact version and reuse a custom program directory.

## Upgrade Docker

From the existing installation folder, with the same Compose project and volume:

```sh
docker compose stop ruphish
```

Take the backup before continuing. Then download the image selected by `compose.yaml` and recreate the service:

```sh
docker compose pull ruphish
docker compose up -d --wait ruphish
docker compose logs --tail=100 ruphish
```

The examples use `ghcr.io/srothgan/ruphish:latest`; pulling updates that tag to the current image. If your deployment pins a release tag or digest, update it deliberately before pulling. Keep the existing data-volume mapping. Do not use `down --volumes` during an upgrade.

## Upgrade the Linux service

Stop the service and take the backup first:

```sh
sudo systemctl stop ruphish
```

Run the normal installer under your own account, then copy the downloaded executable to the service location:

```sh
curl -fsSL https://www.ruphish.com/install.sh | sh
```

Use the installed binary path printed by the installer, or its default command link at `$HOME/.local/bin/ruphish`:

```sh
sudo install -m 0755 "$HOME/.local/bin/ruphish" /usr/local/bin/ruphish
/usr/local/bin/ruphish --version
sudo systemctl start ruphish
sudo journalctl -u ruphish -n 100 --no-pager
```

Using the explicit new-binary path avoids accidentally copying the old service executable found earlier on `PATH`.

## Upgrade a foreground native installation

Stop Ruphish with **Ctrl+C**, take the backup, and rerun the installer for your platform:

```sh
curl -fsSL https://www.ruphish.com/install.sh | sh
```

Or in PowerShell:

```powershell
irm https://www.ruphish.com/install.ps1 | iex
```

Check `ruphish --version`, then start it under the same operating-system account with the same configuration as before. If you use `--config`, keep using that option and the same path.

## Verify the upgrade

Wait for `RUPHISH_READY` and check [readiness](health-and-logs.md#check-readiness). Then sign in and inspect shared resources, recent campaign results, delivery state, and integration status. Check public HTTPS and campaign-domain routing from the network recipients use.

Startup time can increase when the database needs changes. A failed readiness check does not by itself justify deleting a database lock or migration record. Read the startup error and follow [Troubleshooting](troubleshoot-the-installation.md).

## If startup rejects the database

Ruphish checks migration history and schema consistency. It accepts recognized supported histories; it does not promise to import every older development database. An unknown history, altered migration checksum, or schema mismatch must be investigated rather than bypassed by editing migration tables.

SQLite upgrades can create a verified `ruphish-pre-upgrade-*.sqlite3` recovery snapshot. Server databases need operator-managed backups. Preserve any recovery path or conversion records reported by startup.

To roll back after a database change, stop the new version and restore the pre-upgrade database, configuration, and matching application release together. Do not assume that an older executable can read a database already changed by a newer one. For Gophish-specific transformations and recovery records, see [Migration verification and rollback](../../migration/verification-and-rollback.md).
